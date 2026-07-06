//! System-related Tauri commands (autostart, window, explorer, pinned, editor)

use crate::command::{lock_state, AppState};
use crate::service::fileop;
use std::sync::{Arc, Mutex};
use tauri::{State, Manager};

// ── Auto Start ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn enable_autostart(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch()
        .enable()
        .map_err(|e| format!("Failed to enable autostart: {}", e))
}

#[tauri::command]
pub fn disable_autostart(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch()
        .disable()
        .map_err(|e| format!("Failed to disable autostart: {}", e))
}

#[tauri::command]
pub fn is_autostart_enabled(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch()
        .is_enabled()
        .map_err(|e| format!("Failed to check autostart status: {}", e))
}

// ── Window ─────────────────────────────────────────────────────────────

#[tauri::command]
pub fn hide_main_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|e| format!("Failed to hide window: {}", e))
    } else {
        log::warn!("cmd::hide_main_window: main window not found");
        Err("Main window not found".to_string())
    }
}

// ── File Explorer ──────────────────────────────────────────────────────

#[tauri::command]
pub fn open_in_explorer(path: String) -> Result<(), String> {
    tauri_plugin_opener::open_path(&path, None::<&str>)
        .map_err(|e| format!("Failed to open in explorer: {}", e))
}

/// Open a URL (http/https/ftp/etc.) with the system default handler.
/// Uses ShellExecuteW on Windows for robust URI dispatch.
#[cfg(windows)]
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    use windows::core::w;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let wide: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            windows::core::PCWSTR(wide.as_ptr()),
            windows::core::PCWSTR::null(),
            windows::core::PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if (result.0 as isize) > 32 {
        Ok(())
    } else {
        Err(format!("ShellExecuteW failed (code {})", result.0 as isize))
    }
}

#[cfg(not(windows))]
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    tauri_plugin_opener::open_path(&url, None::<&str>)
        .map_err(|e| format!("Failed to open url: {}", e))
}

/// Check whether a path is a file, directory, or doesn't exist.
#[tauri::command]
pub fn get_path_type(path: String) -> Result<String, String> {
    let p = std::path::Path::new(&path);
    if p.is_file() {
        Ok("file".into())
    } else if p.is_dir() {
        Ok("dir".into())
    } else {
        Ok("not_found".into())
    }
}

// ── Debug log ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn debug_log(msg: String) {
    log::debug!("[FE] {}", msg);
}

// ── DevTools ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn open_devtools(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.open_devtools();
        Ok(())
    } else {
        Err("Main window not found".to_string())
    }
}

// ── Open in Editor ────────────────────────────────────────────────────

#[tauri::command]
pub fn open_in_editor(
    state: State<'_, Arc<Mutex<AppState>>>,
    id: i64,
) -> Result<(), String> {
    let content = {
        let s = lock_state!(state);
        s.history.get_entry_content(id).map_err(|e| e.to_string())?
    };
    fileop::Service::preview_text(&content)
}

// ── Pinned ─────────────────────────────────────────────────────────────

#[tauri::command]
pub fn toggle_pinned(
    app: tauri::AppHandle,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<bool, String> {
    let s = lock_state!(state);
    let mut pinned = s.pinned.lock().map_err(|e| e.to_string())?;
    *pinned = !*pinned;
    let new_val = *pinned;
    log::info!("cmd::toggle_pinned: new value={}", new_val);
    // Actually set window always-on-top
    if let Some(window) = app.get_webview_window("main") {
        if let Err(e) = window.set_always_on_top(new_val) {
            log::warn!("toggle_pinned: set_always_on_top failed: {}", e);
        }
    }
    Ok(new_val)
}

#[tauri::command]
pub fn get_pinned(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<bool, String> {
    let s = lock_state!(state);
    let pinned = s.pinned.lock().map_err(|e| e.to_string())?;
    log::info!("cmd::get_pinned: value={}", *pinned);
    Ok(*pinned)
}
