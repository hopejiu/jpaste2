//! Toast notification window management.
//!
//! Responsible for creating/reusing the frameless toast window, content
//! dedup, dynamic sizing, bottom-right positioning, and lifecycle
//! (3s hide → 3min cleanup close).

use crate::model::EVENT_TOAST_SHOW;
use std::sync::atomic::AtomicU64;
use std::sync::Mutex;
use tauri::{Emitter, Manager, WebviewWindowBuilder};

// ── Types ───────────────────────────────────────────────────────────────

/// Data returned from `AppClipboardHandler::process_with_pipeline`
/// for toast creation.
pub(crate) struct ToastPayload {
    pub(crate) message: String,
    pub(crate) icon: String,
    pub(crate) entry_id: i64,
    pub(crate) text: String,
    pub(crate) actions: Vec<String>,
}

// ── State ───────────────────────────────────────────────────────────────

const TOAST_LABEL: &str = "toast-0";

struct ToastState {
    last_hash: Mutex<Option<String>>,
    /// Single generation counter: incremented on each new toast.
    /// Timer threads use this to check if they're still the latest:
    /// - 3s display thread: hides the window if gen still matches
    /// - 3min cleanup thread: closes the window if gen still matches
    gen: AtomicU64,
}

impl ToastState {
    const fn new() -> Self {
        Self {
            last_hash: Mutex::new(None),
            gen: AtomicU64::new(0),
        }
    }

    fn should_suppress(&self, message: &str) -> bool {
        let mut last = self.last_hash.lock().unwrap();
        if let Some(ref prev) = *last {
            if prev == message {
                return true;
            }
        }
        *last = Some(message.to_string());
        false
    }

    fn next_gen(&self) -> u64 {
        self.gen.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1
    }

    fn is_current(&self, gen: u64) -> bool {
        self.gen.load(std::sync::atomic::Ordering::SeqCst) == gen
    }

    fn clear(&self) {
        let mut last = self.last_hash.lock().unwrap();
        *last = None;
    }
}

static TOAST_STATE: ToastState = ToastState::new();

// ── Helpers ─────────────────────────────────────────────────────────────

/// Percent-encode a string for use as a URL query parameter value.
fn percent_encode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// Get the work area (monitor area excluding taskbar) for the monitor at
/// the given position. Returns (x, y, width, height) in physical pixels.
#[cfg(windows)]
fn get_work_area_at(x: i32, y: i32) -> Option<(i32, i32, u32, u32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTONEAREST};
    unsafe {
        let point = POINT { x, y };
        let hmonitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        let mut info = windows::Win32::Graphics::Gdi::MONITORINFO {
            cbSize: std::mem::size_of::<windows::Win32::Graphics::Gdi::MONITORINFO>() as u32,
            ..Default::default()
        };
        if windows::Win32::Graphics::Gdi::GetMonitorInfoW(hmonitor, &mut info).as_bool() {
            let rc = info.rcWork;
            Some((
                rc.left,
                rc.top,
                (rc.right - rc.left) as u32,
                (rc.bottom - rc.top) as u32,
            ))
        } else {
            None
        }
    }
}

#[cfg(not(windows))]
fn get_work_area_at(_x: i32, _y: i32) -> Option<(i32, i32, u32, u32)> {
    None
}

// ── Public API ──────────────────────────────────────────────────────────

pub(crate) fn create_toast_window_inner(
    app: &tauri::AppHandle, message: &str, title: &str, icon: &str,
    entry_id: i64, full_text: &str, actions: &[String],
) {
    // Dedup: skip if same message as last toast
    if TOAST_STATE.should_suppress(message) {
        return;
    }

    // Dynamic height: 70px no actions, 110px 1 action, 130px 2-3 actions
    let height: f64 = match actions.len() {
        0 => 70.0,
        1 => 110.0,
        _ => 130.0,
    };

    let encoded_title = percent_encode(title);
    let encoded_msg = percent_encode(message);
    let encoded_text = percent_encode(&crate::util::truncate(full_text, 1024));
    let encoded_actions = percent_encode(&actions.join(","));
    let hash = format!(
        "/toast?title={}&message={}&icon={}&id={}&text={}&actions={}",
        encoded_title, encoded_msg, icon, entry_id, encoded_text, encoded_actions,
    );

    // Get or create the toast window
    let window = if let Some(existing) = app.get_webview_window(TOAST_LABEL) {
        // Reuse existing hidden window — push new content via event
        let _ = existing.emit(
            EVENT_TOAST_SHOW,
            serde_json::json!({
                "title": title, "message": message, "icon": icon,
                "id": entry_id, "text": full_text, "actions": actions,
            }),
        );
        // Resize for the new content (Logical to match builder's inner_size)
        let _ = existing.set_size(tauri::Size::Logical(tauri::LogicalSize::new(340.0, height)));
        // Eval hash as fallback for webview reloads
        let eval_js = format!(
            "window.__INITIAL_HASH__ = '{}'; window.location.hash = '{}'; try{{sessionStorage.setItem('__TOAST_WINDOW__','1')}}catch(e){{}}",
            hash, hash
        );
        let _ = existing.eval(&eval_js);
        existing
    } else {
        // Create a fresh toast window
        let url = crate::command::viewer::resolve_window_url(&app.config());
        let init_script = format!(
            "window.__INITIAL_HASH__ = '{}'; window.location.hash = '{}'; try{{sessionStorage.setItem('__TOAST_WINDOW__','1')}}catch(e){{}}",
            hash, hash
        );

        let window = match WebviewWindowBuilder::new(app, TOAST_LABEL, url)
            .title("")
            .inner_size(340.0, height)
            .min_inner_size(340.0, 70.0)
            .max_inner_size(340.0, 200.0)
            .resizable(false)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .visible(false)
            .initialization_script(&init_script)
            .transparent(true)
            .focusable(false)
            .focused(false)
            .build()
        {
            Ok(w) => w,
            Err(e) => {
                log::error!("toast: window build FAILED: {}", e);
                return;
            }
        };

        // Fallback eval for initial content
        let eval_js = format!(
            "window.__INITIAL_HASH__ = '{}'; window.location.hash = '{}'; try{{sessionStorage.setItem('__TOAST_WINDOW__','1')}}catch(e){{}}",
            hash, hash
        );
        let _ = window.eval(&eval_js);

        window
    };

    // Position bottom-right, 10px from work area edges (on every show)
    if let Some(monitor) = window.current_monitor().ok().flatten() {
        let size = monitor.size();
        let mon_pos = monitor.position();
        let scale = monitor.scale_factor();
        let center_x = mon_pos.x + (size.width as i32) / 2;
        let center_y = mon_pos.y + (size.height as i32) / 2;
        let (wa_x, wa_y, wa_w, wa_h) = get_work_area_at(center_x, center_y).unwrap_or((
            mon_pos.x,
            mon_pos.y,
            size.width,
            size.height,
        ));
        let px = wa_x + (wa_w as f64 - (340.0 + 10.0) * scale).round() as i32;
        let py = wa_y + (wa_h as f64 - (height + 10.0) * scale).round() as i32;
        let _ = window.set_position(tauri::PhysicalPosition::new(px, py));
    }

    // Show without set_focus — toast must not steal keyboard focus.
    let _ = window.show();

    // Lifecycle: 3s display → hide → 3min idle → close
    let my_gen = TOAST_STATE.next_gen();
    let app_clone = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3));
        if TOAST_STATE.is_current(my_gen) {
            if let Some(w) = app_clone.get_webview_window(TOAST_LABEL) {
                let _ = w.hide();
            }
            TOAST_STATE.clear();

            let cleanup_gen = TOAST_STATE.next_gen();
            let app_for_cleanup = app_clone.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(180));
                if TOAST_STATE.is_current(cleanup_gen) {
                    if let Some(w) = app_for_cleanup.get_webview_window(TOAST_LABEL) {
                        let _ = w.close();
                    }
                }
            });
        }
    });
}

#[tauri::command]
pub(crate) fn show_toast(app: tauri::AppHandle, message: String) -> Result<(), String> {
    // Spawn on a separate thread — WebviewWindowBuilder::build() can block
    std::thread::spawn(move || {
        create_toast_window_inner(&app, &message, "jPaste", "clipboard", 0, "", &[]);
    });
    Ok(())
}
