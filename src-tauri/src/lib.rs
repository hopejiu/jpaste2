//! jPaste v2 — Tauri Application Entry
//!
//! Responsible for: plugin registration, service wiring, window setup.
//! All Tauri commands are in the `command/` module.

mod clipboard;
mod command;
mod log_service;
mod model;
mod monitor;
mod net;
mod qrcode;
mod repository;
mod service;
mod toast;
mod util;

#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::sync::Mutex;
use tauri::{Emitter, Listener, Manager, WindowEvent};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

use crate::model::{EVENT_WINDOW_HIDING, EVENT_WINDOW_SHOWN};
use clipboard::{start_watcher, ClipboardContent, ClipboardEventHandler, ClipboardManager};
use command::AppState;

/// Handler that bridges clipboard events to Tauri app state.
/// Uses a channel to dispatch events to a fixed worker thread (no per-event spawn).
struct AppClipboardHandler {
    sender: std::sync::mpsc::Sender<ClipboardContent>,
}

impl AppClipboardHandler {
    /// Create the handler and start the background worker thread.
    fn new(state: Arc<Mutex<AppState>>, app_handle: Option<tauri::AppHandle>) -> Self {
        let (sender, receiver) = std::sync::mpsc::channel::<ClipboardContent>();
        let state_clone = state;
        let handle_clone = app_handle;

        // Single worker thread avoids spawn overhead per copy
        std::thread::Builder::new()
            .name("clipboard-worker".into())
            .spawn(move || {
                while let Ok(content) = receiver.recv() {
                    let handle_ref = handle_clone.as_ref();
                    let toast_info =
                        monitor::capture::build_toast(&state_clone, handle_ref, &content);

                    match toast_info {
                        Some(info) => {
                            log::debug!(
                                "clipboard-worker: DISPATCHING toast — message={:?} icon={:?} entry_id={} actions={:?}",
                                info.message, info.icon, info.entry_id, info.actions
                            );
                            if let Some(app) = &handle_clone {
                                toast::create_toast_window_inner(
                                    app, &info.message, "jPaste", &info.icon,
                                    info.entry_id, &info.text, &info.actions,
                                );
                            }
                        }
                        None => {
                            log::debug!("clipboard-worker: no toast to show for this clipboard event");
                        }
                    }
                }
                log::debug!("clipboard-worker: channel closed, exiting");
            })
            .expect("failed to spawn clipboard worker thread");

        Self { sender }
    }

}

impl ClipboardEventHandler for AppClipboardHandler {
    fn on_clipboard_change(&self, content: ClipboardContent) {
        // Non-blocking send — drops event if worker is backed up
        if let Err(e) = self.sender.send(content) {
            log::warn!("clipboard: failed to send event to worker: {}", e);
        }
    }
}

// ── Setup helpers ───────────────────────────────────────────────────────

type HotkeyCell = Arc<std::sync::Mutex<Option<tauri::AppHandle>>>;
type StateCell = Arc<std::sync::Mutex<Option<Arc<Mutex<AppState>>>>>;

fn build_services(
    app_data: &std::path::Path,
    app_handle: &tauri::AppHandle,
) -> (Arc<Mutex<AppState>>, log_service::LogService) {
    let log_svc = log_service::init_global_logger(app_data, 12);
    log::info!("Log service initialized");

    let history = crate::service::history::HistoryService::new(app_data)
        .expect("failed to initialize history service");
    let settings = crate::service::settings::SettingsService::new(app_data);

    let self_write_tracker = Arc::new(Mutex::new(crate::util::SelfWriteTracker::new()));

    let filostack = crate::service::filostack::FiloStack::with_shared_tracker(
        self_write_tracker.clone(),
    );
    let clipboard_mgr = Arc::new(Mutex::new(
        ClipboardManager::with_shared_tracker(self_write_tracker.clone())
            .expect("failed to initialize clipboard"),
    ));

    if let Err(e) = settings.load() {
        log::warn!("Failed to load settings: {}", e);
    }

    // Build launch hotkey map from loaded settings via the shared helper.
    let launch_hotkey_map = {
        let targets = settings.get_launch_targets().unwrap_or_default();
        crate::command::quicklaunch::build_launch_hotkey_map(&targets)
    };

    let state = Arc::new(Mutex::new(AppState {
        history,
        settings,
        filostack,
        clipboard_mgr: clipboard_mgr.clone(),
        app_handle: Some(app_handle.clone()),
        keyboard_hook: monitor::hook::KeyboardHook::new(),
        ctrl_v_sender: Mutex::new(None),
        pinned: Mutex::new(false),
        launch_hotkey_map: Mutex::new(launch_hotkey_map),
    }));

    (state, log_svc)
}

fn setup_clipboard_watcher(state: &Arc<Mutex<AppState>>, app_handle: &tauri::AppHandle) {
    let handler = AppClipboardHandler::new(state.clone(), Some(app_handle.clone()));
    log::info!("Starting clipboard watcher...");
    let clipboard_mgr = {
        let s = state.lock().unwrap();
        s.clipboard_mgr.clone()
    };
    match start_watcher(clipboard_mgr, Arc::new(handler)) {
        Ok(()) => log::info!("Clipboard watcher started successfully"),
        Err(e) => log::error!("Failed to start clipboard watcher: {}", e),
    }
}

fn setup_hotkeys(
    app_handle: &tauri::AppHandle,
    state: &Arc<Mutex<AppState>>,
    app_handle_for_hotkey: &HotkeyCell,
    state_for_hotkey: &StateCell,
) {
    *app_handle_for_hotkey.lock().unwrap() = Some(app_handle.clone());
    *state_for_hotkey.lock().unwrap() = Some(state.clone());

    let (hk_str, toolbox_hotkeys) = {
        let s = state.lock().unwrap();
        let settings = s.settings.get_settings().ok();
        (
            settings.as_ref().map(|d| d.hotkey.clone()).unwrap_or_else(|| "Alt+V".to_string()),
            settings.map(|d| d.toolbox_hotkeys.clone()).unwrap_or_default(),
        )
    };

    // Register main clipboard hotkey
    if let Ok(shortcut) = hk_str.parse::<tauri_plugin_global_shortcut::Shortcut>() {
        let _ = app_handle.global_shortcut().register(shortcut);
    }

    // Register all enabled launch target hotkeys
    let launch_hotkeys: Vec<String> = {
        let s = state.lock().unwrap();
        let map = s.launch_hotkey_map.lock().unwrap();
        map.keys().cloned().collect()
    };
    for hk in &launch_hotkeys {
        if let Ok(s) = hk.parse::<tauri_plugin_global_shortcut::Shortcut>() {
            let _ = app_handle.global_shortcut().register(s);
        }
    }

    // Register all toolbox hotkeys
    for (route, hk) in &toolbox_hotkeys {
        if let Ok(s) = hk.parse::<tauri_plugin_global_shortcut::Shortcut>() {
            let _ = app_handle.global_shortcut().register(s);
            log::debug!("hotkey: registered toolbox {} → {}", route, hk);
        } else {
            log::warn!("hotkey: invalid toolbox hotkey {} for {}", hk, route);
        }
    }

    // Settings change callback: diff main hotkey + toolbox hotkeys
    let app_handle_clone = app_handle.clone();
    let s = state.lock().unwrap();
    s.settings.on_settings_change(move |old, new| {
        // Diff main hotkey
        if old.hotkey != new.hotkey {
            if let Ok(old_s) = old.hotkey.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                let _ = app_handle_clone.global_shortcut().unregister(old_s);
            }
            if !new.hotkey.trim().is_empty() {
                if let Ok(new_s) = new.hotkey.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                    app_handle_clone
                        .global_shortcut()
                        .register(new_s)
                        .map_err(|e| format!("快捷键注册失败: {}", e))?;
                } else {
                    return Err(format!("无效的快捷键: {}", new.hotkey));
                }
            }
            log::info!("hotkey: main changed '{}' → '{}'", old.hotkey, new.hotkey);
        }

        // Diff toolbox hotkeys
        if old.toolbox_hotkeys != new.toolbox_hotkeys {
            // Unregister removed/changed
            for (route, old_hk) in &old.toolbox_hotkeys {
                let new_hk = new.toolbox_hotkeys.get(route);
                if new_hk.map(|h| h.as_str()) != Some(old_hk.as_str()) {
                    if let Ok(s) = old_hk.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                        let _ = app_handle_clone.global_shortcut().unregister(s);
                        log::debug!("hotkey: unregistered toolbox {} → {}", route, old_hk);
                    }
                }
            }
            // Register added/changed
            for (route, new_hk) in &new.toolbox_hotkeys {
                let old_hk = old.toolbox_hotkeys.get(route);
                if old_hk.map(|h| h.as_str()) != Some(new_hk.as_str()) {
                    if let Ok(s) = new_hk.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                        app_handle_clone
                            .global_shortcut()
                            .register(s)
                            .map_err(|e| format!("工具箱快捷键注册失败 ({}): {}", route, e))?;
                        log::debug!("hotkey: registered toolbox {} → {}", route, new_hk);
                    }
                }
            }
        }

        Ok(())
    });
}

fn setup_window_visibility(state: &Arc<Mutex<AppState>>, app_handle: &tauri::AppHandle) {
    let start_minimized = state
        .lock()
        .ok()
        .and_then(|s| s.settings.get_settings().ok())
        .map(|d| d.start_minimized)
        .unwrap_or(false);
    if start_minimized {
        log::info!("start_minimized: window stays hidden");
    } else {
        log::info!("start_minimized: showing window");
        if let Some(window) = app_handle.get_webview_window("main") {
            if let Ok(s) = state.lock() {
                if let Ok(settings) = s.settings.get_settings() {
                    if settings.center_on_show {
                        center_main_window(&window);
                    }
                }
            }
            crate::show_focus_window(&window);
        }
    }
}

/// Initialize the application
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    eprintln!("[jPaste] starting...");

    // ponytail: panic = "abort" (release profile) means ANY thread panic
    // terminates the whole process with no log. This hook records the panic
    // message + location + backtrace to the log file BEFORE the abort, so
    // "occasional silent auto-exit" leaves a trace to analyze.
    std::panic::set_hook(Box::new(|info| {
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "<non-string panic payload>".to_string()
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown location>".to_string());
        let thread_name = std::thread::current()
            .name()
            .unwrap_or("<unnamed>")
            .to_string();
        let bt = std::backtrace::Backtrace::force_capture();
        let msg = format!(
            "\n[CRASH] thread '{}' panicked at {}: {}\n[CRASH] backtrace:\n{}\n",
            thread_name, location, payload, bt
        );
        crate::log_service::crash_log(&msg);
    }));

    let app_handle_for_hotkey: HotkeyCell = Arc::new(std::sync::Mutex::new(None));
    let app_handle_for_hotkey_clone = app_handle_for_hotkey.clone();

    let state_for_hotkey: StateCell = Arc::new(std::sync::Mutex::new(None));
    let state_for_hotkey_clone = state_for_hotkey.clone();

    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |_app, shortcut, event| {
                    use tauri_plugin_global_shortcut::ShortcutState;
                    if matches!(event.state, ShortcutState::Pressed) {
                        let hk_str = shortcut.to_string();

                        // Check launch target hotkeys
                        if let Some(ref state_lock) = *state_for_hotkey_clone.lock().unwrap() {
                            if let Ok(s) = state_lock.lock() {
                                let map = s.launch_hotkey_map.lock().unwrap();
                                if let Some(target_id) = map.get(&hk_str) {
                                    let target_id = target_id.clone();
                                    let app_clone = if let Some(ref h) = *app_handle_for_hotkey_clone.lock().unwrap() {
                                        h.clone()
                                    } else {
                                        return;
                                    };
                                    drop(map); drop(s);
                                    if let Ok(s) = state_lock.lock() {
                                        let _ = crate::command::quicklaunch::launch_target_by_hotkey(
                                            &app_clone, &s, &target_id,
                                        );
                                    }
                                    return;
                                }

                                // Check toolbox hotkeys
                                if let Ok(settings) = s.settings.get_settings() {
                                    for (route, hk) in &settings.toolbox_hotkeys {
                                        if let Ok(s) = hk.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                                            if s.to_string() == hk_str {
                                                if let Some(ref h) = *app_handle_for_hotkey_clone.lock().unwrap() {
                                                    crate::command::toolbox::open_toolbox_item(h, route);
                                                }
                                                return;
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Fallback: clipboard toggle (Alt+V)
                        if let Some(ref app_handle) = *app_handle_for_hotkey_clone.lock().unwrap() {
                            if let Some(window) = app_handle.get_webview_window("main") {
                                if window.is_visible().unwrap_or(false) {
                                    let _ = window.hide();
                                } else {
                                    if let Some(ref state_lock) =
                                        *state_for_hotkey_clone.lock().unwrap()
                                    {
                                        if let Ok(s) = state_lock.lock() {
                                            if let Ok(settings) = s.settings.get_settings() {
                                                if settings.center_on_show {
                                                    center_main_window(&window);
                                                }
                                            }
                                        }
                                    }
                                    crate::show_focus_window(&window);
                                }
                            }
                        }
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None::<Vec<&str>>,
        ))
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cmd| {
            if let Some(window) = app.get_webview_window("main") {
                crate::show_focus_window(&window);
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let app_handle = app.handle().clone();

            let app_data = app
                .path()
                .app_data_dir()
                .expect("failed to get app data dir");
            std::fs::create_dir_all(&app_data)?;

            let (state, log_svc) = build_services(&app_data, &app_handle);
            log::info!("Log file: {}", app_data.join("logs").display());

            // ShareServer managed state (independent of AppState, SoC).
            let share_state: Arc<Mutex<crate::command::share_server::ShareState>> =
                Arc::new(Mutex::new(crate::command::share_server::ShareState::default()));

            setup_clipboard_watcher(&state, &app_handle);
            setup_hotkeys(&app_handle, &state, &app_handle_for_hotkey, &state_for_hotkey);
            setup_system_tray(&app_handle, app_handle.clone());

            // Window visibility first: decide whether to show BEFORE registering
            // window event handlers, so WebView2 init triggered by on_window_event
            // can't interfere with the start_minimized decision.
            setup_window_visibility(&state, &app_handle);

            // Pre-create the hidden toast window so every real toast reuses it
            // (avoids the cold-create + show race that flashes the first
            // image/QR toast on Windows).
            toast::ensure_toast_window(&app_handle);

            // Window event handlers — registered AFTER show so stale
            // Focused(true) events from the initial show/set_focus don't
            // get caught here and interfere with startup logic.
            setup_window_behavior(&app_handle, state.clone());
            setup_cleanup_timer(state.clone());

            // ShareServer lifecycle: the panel registers its own `Destroyed`
            // handler (see open_share_panel) so closing it stops the service.
            setup_log_relay(&app_handle, log_svc);

            // Clean up temp directory on app exit
            let _app_handle = app_handle.clone();
            app_handle.listen("tauri://close-requested", move |_| {
                crate::util::cleanup_temp_dir();
            });

            app.manage(state);
            app.manage(share_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // History
            command::history::get_entries,
            command::history::get_entry_content,
            command::history::get_entry_image,
            command::history::get_entry_image_full,
            command::history::delete_entry,
            command::history::toggle_favorite,
            command::history::cleanup,
            command::history::clear_all,
            command::history::get_stats,
            command::history::get_image_list,
            command::history::get_entries_regex,
            command::history::increment_copy_count,
            command::history::scan_qr_text,
            // Settings
            command::settings::get_settings,
            command::settings::save_settings,
            // FiloStack
            command::filostack::get_filo_status,
            command::filostack::filo_set_mode,
            command::filostack::filo_clear,
            // Viewer windows (single registry-driven command)
            command::viewer::open_viewer,
            // Clipboard
            command::clipboard::set_clipboard_text,
            command::clipboard::copy_entry,
            command::clipboard::paste_entry,
            command::clipboard::paste_entry_and_hide,
            toast::show_toast,
            // Image generation & export (toolbox)
            command::image_export::generate_qr,
            command::image_export::write_clipboard_image,
            command::image_export::get_clipboard_text,
            command::image_export::save_image_dialog,
            // File operations
            command::system::open_in_explorer,
            command::system::open_url,
            command::system::open_in_editor,
            command::system::get_path_type,
            // Auto start
            command::system::enable_autostart,
            command::system::disable_autostart,
            command::system::is_autostart_enabled,
            // Window
            command::system::hide_main_window,
            command::system::open_devtools,
            // Curl
            command::curl::send_curl_request,
            // Pinned
            command::system::toggle_pinned,
            command::system::get_pinned,
            // Debug
            command::system::debug_log,
            // QuickLaunch
            command::quicklaunch::get_launch_targets,
            command::quicklaunch::save_launch_targets,
            command::quicklaunch::launch_target,
            command::quicklaunch::check_target_hotkey,
            command::quicklaunch::pick_file_path,
            command::quicklaunch::open_quicklaunch,
            // ShareServer (HTTP LAN sharing)
            command::share_server::open_share_panel,
            command::share_server::start_share_server,
            command::share_server::stop_share_server,
            command::share_server::pick_share_files,
            command::share_server::add_share_file,
            command::share_server::add_share_text,
            command::share_server::remove_share_item,
            command::share_server::list_share_items,
            command::share_server::get_share_urls,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ── Window helpers ────────────────────────────────────────────────────

/// Show a window and bring it to the foreground.
/// No-op on error (caller doesn't care about window-manager failures).
pub(crate) fn show_focus_window(window: &tauri::WebviewWindow) {
    let _ = window.show();
    let _ = window.set_focus();
}

/// Start an auto-hide timer: if `cancel_rx` receives nothing within
/// `delay_ms`, hide the window identified by `label`.
/// Returns a `Sender` that, when sent to or dropped, cancels the timer.
/// Used together with `FocusEvent::Focused(true)` to cancel pending hides.
pub(crate) fn start_autohide_timer(
    handle: &tauri::AppHandle,
    label: &str,
    delay_ms: u64,
) -> std::sync::mpsc::Sender<()> {
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let app = handle.clone();
    let l = label.to_string();
    std::thread::spawn(move || {
        match rx.recv_timeout(std::time::Duration::from_millis(delay_ms)) {
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if let Some(w) = app.get_webview_window(&l) {
                    let _ = w.hide();
                }
            }
            _ => {} // Disconnected (sender dropped) or Ok — do nothing
        }
    });
    tx
}

// ── Window Behavior ───────────────────────────────────────────────────

/// Center the main window on its current monitor in physical coordinates.
/// Safe to call on a hidden window — position takes effect before first paint.
fn center_main_window(window: &tauri::WebviewWindow) {
    if let Some(monitor) = window.current_monitor().ok().flatten() {
        let size = monitor.size();
        let mon_pos = monitor.position();
        if let Ok(ws) = window.outer_size() {
            let phys_x = mon_pos.x + ((size.width as i32) - (ws.width as i32)) / 2;
            let phys_y = mon_pos.y + ((size.height as i32) - (ws.height as i32)) / 2;
            let _ = window.set_position(tauri::PhysicalPosition::new(
                phys_x.max(0),
                phys_y.max(0),
            ));
        }
    }
}

fn setup_window_behavior(app_handle: &tauri::AppHandle, state: Arc<Mutex<AppState>>) {
    let was_hidden: std::sync::Arc<std::sync::atomic::AtomicBool> =
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    let pending_hide: std::sync::Arc<std::sync::Mutex<Option<std::sync::mpsc::Sender<()>>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    if let Some(window) = app_handle.get_webview_window("main") {
        let handle = app_handle.clone();
        let was_hidden_clone = was_hidden.clone();
        let state_clone = state.clone();
        window.on_window_event(move |event| {
            match event {
                WindowEvent::Focused(false) => {
                    let should_hide = if let Ok(s) = state_clone.lock() {
                        if let Ok(p) = s.pinned.lock() {
                            log::info!("setup_window_behavior: Focused(false) pinned={}", *p);
                            !*p
                        } else {
                            true
                        }
                    } else {
                        true
                    };
                    if !should_hide {
                        log::info!("setup_window_behavior: pinned=true, skipping auto-hide");
                        return;
                    }

                    let handle_clone = handle.clone();
                    let wh = was_hidden_clone.clone();
                    let (tx, rx) = std::sync::mpsc::channel::<()>();
                    if let Ok(mut p) = pending_hide.lock() {
                        *p = Some(tx);
                    }

                    std::thread::spawn(move || {
                        if rx
                            .recv_timeout(std::time::Duration::from_millis(150))
                            .is_err()
                        {
                            let _ = handle_clone.emit(EVENT_WINDOW_HIDING, ());
                            if let Some(w) = handle_clone.get_webview_window("main") {
                                let _ = w.hide();
                            }
                            wh.store(true, std::sync::atomic::Ordering::SeqCst);
                        }
                    });
                }
                WindowEvent::Focused(true) => {
                    // Prime the toast window's first show while the app is
                    // foreground (covers start_minimized / tray launch where
                    // the app isn't foreground at startup). Idempotent.
                    toast::prime_toast_window(&handle);

                    if let Ok(mut p) = pending_hide.lock() {
                        if let Some(tx) = p.take() {
                            let _ = tx.send(());
                        }
                    }

                    if was_hidden_clone.load(std::sync::atomic::Ordering::SeqCst) {
                        if let Some(main_window) = handle.get_webview_window("main") {
                            let _ = main_window.eval("window.location.hash = '/';");
                        }
                    }

                    let _ = handle.emit(EVENT_WINDOW_SHOWN, ());

                    if was_hidden_clone.swap(false, std::sync::atomic::Ordering::SeqCst) {
                        let should_center = state_clone
                            .lock()
                            .ok()
                            .and_then(|s| s.settings.get_settings().ok())
                            .map(|settings| settings.center_on_show)
                            .unwrap_or(false);
                        if should_center {
                            if let Some(w) = handle.get_webview_window("main") {
                                center_main_window(&w);
                            }
                        }
                    }
                }
                _ => {}
            }
        });
    }
}

fn setup_cleanup_timer(state: Arc<Mutex<AppState>>) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(60 * 60));
        if let Ok(s) = state.lock() {
            if let Ok(settings) = s.settings.get_settings() {
                let retain_days = settings.retain_days;
                if let Err(e) = s.history.cleanup(retain_days) {
                    log::warn!("Scheduled cleanup failed: {}", e);
                }
            }
        }
    });
}

fn setup_log_relay(app_handle: &tauri::AppHandle, log_svc: log_service::LogService) {
    let log_svc_clone = log_svc;
    app_handle.listen("frontend-log", move |event| {
        let payload_str = event.payload();
        if let Ok(payload) = serde_json::from_str::<serde_json::Value>(payload_str) {
            let level = payload
                .get("level")
                .and_then(|v| v.as_str())
                .unwrap_or("info");
            let component = payload
                .get("component")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let msg = payload.get("msg").and_then(|v| v.as_str()).unwrap_or("");
            log_svc_clone.frontend_log(level, component, msg);
        } else {
            log::warn!("frontend-log: failed to parse payload: {}", payload_str);
        }
    });
}

// ── System Tray ────────────────────────────────────────────────────────

fn setup_system_tray(app: &tauri::AppHandle, _app_handle: tauri::AppHandle) {
    let _tray = app.tray_by_id("main").unwrap_or_else(|| {
        use tauri::menu::{MenuBuilder, MenuItemBuilder};
        use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

        let show_item = MenuItemBuilder::with_id("show", "显示").build(app).unwrap();
        let settings_item = MenuItemBuilder::with_id("settings", "设置")
            .build(app)
            .unwrap();
        let quit_item = MenuItemBuilder::with_id("quit", "退出").build(app).unwrap();

        let menu = MenuBuilder::new(app)
            .item(&show_item)
            .item(&settings_item)
            .separator()
            .item(&quit_item)
            .build()
            .unwrap();

        TrayIconBuilder::new()
            .icon(app.default_window_icon().unwrap().clone())
            .tooltip("jpaste \n快捷剪贴板工具")
            .menu(&menu)
            .on_menu_event(move |app, event| match event.id.as_ref() {
                "show" => {
                    if let Some(w) = app.get_webview_window("main") {
                        crate::show_focus_window(&w);
                    }
                }
                "settings" => {
                    if let Some(w) = app.get_webview_window("main") {
                        crate::show_focus_window(&w);
                        let _ = w.eval("window.location.hash = '/settings';");
                    }
                }
                "quit" => {
                    crate::util::cleanup_temp_dir();
                    app.exit(0);
                }
                _ => {}
            })
            .on_tray_icon_event(|tray, event| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
                {
                    let app = tray.app_handle();
                    if let Some(w) = app.get_webview_window("main") {
                        if w.is_visible().unwrap_or(false) {
                            let _ = w.hide();
                        } else {
                            crate::show_focus_window(&w);
                        }
                    }
                }
            })
            .build(app)
            .unwrap()
    });
}
