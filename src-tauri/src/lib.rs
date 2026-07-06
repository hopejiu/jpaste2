//! jPaste v2 — Tauri Application Entry
//!
//! Responsible for: plugin registration, service wiring, window setup.
//! All Tauri commands are in the `command/` module.

mod clipboard;
mod command;
mod hook;
mod log_service;
mod model;
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

use crate::model::{
    EVENT_CLIPBOARD_UPDATED, EVENT_PASTE_ORDER_CHANGED,
    EVENT_WINDOW_HIDING, EVENT_WINDOW_SHOWN,
};
use clipboard::pipeline::ClipboardPipeline;
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
                        Self::process_with_pipeline(&state_clone, handle_ref, &content);

                    match toast_info {
                        Some(info) => {
                            if let Some(app) = &handle_clone {
                                toast::create_toast_window_inner(
                                    app, &info.message, "jPaste", &info.icon,
                                    info.entry_id, &info.text, &info.actions,
                                );
                            }
                        }
                        None => {}
                    }
                }
                log::debug!("clipboard-worker: channel closed, exiting");
            })
            .expect("failed to spawn clipboard worker thread");

        Self { sender }
    }

    fn process_with_pipeline(
        state: &Arc<Mutex<AppState>>,
        handle: Option<&tauri::AppHandle>,
        content: &ClipboardContent,
    ) -> Option<toast::ToastPayload> {
        // Skip saving if clipboard is completely empty
        if content.text.is_empty() && !content.has_image && !content.has_file_uri {
            return None;
        }

        // Resolve image bytes: prefer in-memory, fallback to temp file
        let image_bytes = if content.image_data.is_some() {
            content.image_data.clone()
        } else if let Some(ref path) = content.image_temp_path {
            std::fs::read(path).ok()
        } else {
            None
        };

        let hash = if let Some(ref img) = image_bytes {
            util::sha256_bytes(img)
        } else {
            util::sha256_hex(&content.text)
        };

        // Decode QR code from image at capture time (before pipeline.process)
        let qr_text = if content.has_image {
            if let Some(ref img) = image_bytes {
                qrcode::decode_qr_from_image(img).unwrap_or_default()
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let pipeline = ClipboardPipeline::new(state.clone());

        let result = pipeline.process(
            &content.text,
            content.has_image,
            content.has_file_uri,
            &hash,
            image_bytes.as_deref(),
            &qr_text,
        );

        // Clean up temp file after processing (image is now saved to final location)
        if let Some(ref path) = content.image_temp_path {
            let _ = std::fs::remove_file(path);
        }

        match result {
            Ok(payload) => {
                if let Some(app) = handle {
                    let _ = app.emit(
                        EVENT_CLIPBOARD_UPDATED,
                        serde_json::to_value(&payload).unwrap_or_default(),
                    );
                }

                // Push to filo stack (direct state access, no pipeline indirection)
                if !content.text.is_empty() {
                    if let Ok(s) = state.lock() {
                        s.filostack.push(&content.text);
                    }
                }

                // Queue mode auto-exit
                if let Ok(s) = state.lock() {
                    if s.filostack.mode() == "queue" && (content.has_image || content.has_file_uri) {
                        drop(s);
                        log::info!("filostack: auto-exit queue mode due to non-text content");
                        if let Ok(s) = state.lock() {
                            s.filostack.set_mode("normal");
                            if let Ok(mut new_settings) = s.settings.get_settings() {
                                new_settings.paste_order = "normal".to_string();
                                let _ = s.settings.save_settings(new_settings);
                            }
                        }
                        if let Some(app) = handle {
                            let _ = app.emit(EVENT_PASTE_ORDER_CHANGED, "normal");
                        }
                    }
                }

                // Detect actions for toast (only text content, plus QR)
                let mut actions: Vec<String> = if !content.text.is_empty() && !content.has_image {
                    model::detect_actions(&content.text).into_iter().map(|s| s.to_string()).collect()
                } else {
                    Vec::new()
                };

                // ponytail: QR action is the only image-based action.
                // If we add more image-based actions later, extract a shared helper.
                if content.has_image && !qr_text.is_empty() {
                    actions.push("qrcode".to_string());
                }

                // For single file paths: if parent dir doesn't exist, remove the
                // "folder" action — no point offering to open a dead path.
                let lines: Vec<&str> = content.text.lines().collect();
                let is_single_path = lines.len() == 1
                    && (content.has_file_uri || model::is_windows_path(lines[0]));
                if is_single_path {
                    let path = lines[0].trim();
                    if let Some(parent) = std::path::Path::new(path).parent() {
                        if !parent.exists() {
                            actions.retain(|a| a != "folder");
                        }
                    }
                }

                // Collect toast info (direct state access, no pipeline indirection)
                let should_show = content.has_image
                    || content.has_file_uri
                    || (!content.text.is_empty() && state.lock()
                        .ok()
                        .and_then(|s| s.settings.get_settings().ok())
                        .map(|s| s.notify_enabled)
                        .unwrap_or(false));
                if should_show
                {
                    let (message, icon) = if content.has_image {
                        let msg = if !qr_text.is_empty() {
                            format!("图片 · 检测到二维码")
                        } else {
                            "图片".to_string()
                        };
                        (msg, "image".into())
                    } else if content.has_file_uri {
                        let files: Vec<&str> = content.text.lines().collect();
                        let msg = if files.len() <= 1 {
                            files.first()
                                .and_then(|p| std::path::Path::new(p).file_name())
                                .and_then(|n| n.to_str())
                                .map(|n| util::truncate(n, 60))
                                .unwrap_or_else(|| "文件".into())
                        } else {
                            format!("{} 个文件", files.len())
                        };
                        (msg, "document".into())
                    } else if !content.text.is_empty() {
                        let preview = util::truncate(&content.text, 60);
                        if payload.auto_favorited {
                            (format!("{} ⭐ 已自动收藏", preview), "clipboard".into())
                        } else {
                            (preview, "clipboard".into())
                        }
                    } else {
                        return None;
                    };
                    Some(toast::ToastPayload {
                        message,
                        icon,
                        entry_id: payload.id,
                        text: content.text.clone(),
                        actions,
                    })
                } else {
                    None
                }
            }
            Err(e) => {
                log::error!("Failed to save clipboard content: {}", e);
                None
            }
        }
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

    let state = Arc::new(Mutex::new(AppState {
        history,
        settings,
        filostack,
        clipboard_mgr: clipboard_mgr.clone(),
        app_handle: Some(app_handle.clone()),
        keyboard_hook: hook::KeyboardHook::new(),
        ctrl_v_sender: Mutex::new(None),
        pinned: Mutex::new(false),
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

    let hk_str = {
        let s = state.lock().unwrap();
        s.settings
            .get_settings()
            .ok()
            .map(|d| d.hotkey.clone())
            .unwrap_or_else(|| "Alt+V".to_string())
    };
    if let Ok(shortcut) = hk_str.parse::<tauri_plugin_global_shortcut::Shortcut>() {
        let _ = app_handle.global_shortcut().register(shortcut);
    }

    let app_handle_clone = app_handle.clone();
    let s = state.lock().unwrap();
    s.settings.on_hotkey_change(move |old_hk, new_hk| {
        if let Ok(old) = old_hk.parse::<tauri_plugin_global_shortcut::Shortcut>() {
            let _ = app_handle_clone.global_shortcut().unregister(old);
        }
        if let Ok(shortcut) = new_hk.parse::<tauri_plugin_global_shortcut::Shortcut>() {
            app_handle_clone
                .global_shortcut()
                .register(shortcut)
                .map_err(|e| format!("快捷键注册失败: {}", e))?;
        } else {
            return Err(format!("无效的快捷键: {}", new_hk));
        }
        log::info!("hotkey: changed {} → {}", old_hk, new_hk);
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
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

/// Initialize the application
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    eprintln!("[jPaste] starting...");

    let app_handle_for_hotkey: HotkeyCell = Arc::new(std::sync::Mutex::new(None));
    let app_handle_for_hotkey_clone = app_handle_for_hotkey.clone();

    let state_for_hotkey: StateCell = Arc::new(std::sync::Mutex::new(None));
    let state_for_hotkey_clone = state_for_hotkey.clone();

    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |_app, _shortcut, event| {
                    use tauri_plugin_global_shortcut::ShortcutState;
                    if matches!(event.state, ShortcutState::Pressed) {
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
                                    let _ = window.show();
                                    let _ = window.set_focus();
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
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(move |app| {
            let app_handle = app.handle().clone();

            let app_data = app
                .path()
                .app_data_dir()
                .expect("failed to get app data dir");
            std::fs::create_dir_all(&app_data)?;

            let (state, log_svc) = build_services(&app_data, &app_handle);

            setup_clipboard_watcher(&state, &app_handle);
            setup_hotkeys(&app_handle, &state, &app_handle_for_hotkey, &state_for_hotkey);
            setup_system_tray(&app_handle, app_handle.clone());

            // Window visibility first: decide whether to show BEFORE registering
            // window event handlers, so WebView2 init triggered by on_window_event
            // can't interfere with the start_minimized decision.
            setup_window_visibility(&state, &app_handle);

            // Window event handlers — registered AFTER show so stale
            // Focused(true) events from the initial show/set_focus don't
            // get caught here and interfere with startup logic.
            setup_window_behavior(&app_handle, state.clone());
            setup_cleanup_timer(state.clone());
            setup_log_relay(&app_handle, log_svc);

            // Clean up temp directory on app exit
            let _app_handle = app_handle.clone();
            app_handle.listen("tauri://close-requested", move |_| {
                crate::util::cleanup_temp_dir();
            });

            app.manage(state);
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
            command::clipboard::paste_entry,
            command::clipboard::paste_entry_and_hide,
            toast::show_toast,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
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
            .menu(&menu)
            .on_menu_event(move |app, event| match event.id.as_ref() {
                "show" => {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                }
                "settings" => {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.show();
                        let _ = w.set_focus();
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
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                }
            })
            .build(app)
            .unwrap()
    });
}
