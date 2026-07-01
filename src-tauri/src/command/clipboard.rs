//! Clipboard-related Tauri commands

use crate::command::{lock_state, AppState};
use crate::model::EVENT_CLIPBOARD_UPDATED;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, State, Manager};

#[tauri::command]
pub fn set_clipboard_text(
    state: State<'_, Arc<Mutex<AppState>>>,
    text: String,
) -> Result<(), String> {
    let mgr = {
        let s = lock_state!(state);
        s.clipboard_mgr.clone()
    };
    let mut c = mgr.lock().map_err(|e| e.to_string())?;
    c.write_text(&text)
}

#[tauri::command]
pub fn paste_entry(
    state: State<'_, Arc<Mutex<AppState>>>,
    id: i64,
) -> Result<(), String> {
    log::info!("cmd::paste_entry: id={}", id);
    let content = {
        let s = lock_state!(state);
        s.history.get_entry_content(id)?
    };
    log::info!("cmd::paste_entry: got content ({} bytes)", content.len());

    // Increment copy_count for user-triggered paste
    {
        let s = lock_state!(state);
        let threshold = s.settings.get_settings()
            .map(|settings| if settings.auto_fav_on_copy_count { settings.auto_fav_threshold.max(2).min(10) as i64 } else { 0 })
            .unwrap_or(0);
        let _ = s.history.increment_copy_count(id, threshold);
    }

    let mgr = {
        let s = lock_state!(state);
        s.clipboard_mgr.clone()
    };
    {
        let mut c = mgr.lock().map_err(|e| e.to_string())?;
        c.write_text(&content)?;
    }
    log::info!("cmd::paste_entry: clipboard written, simulating paste...");
    crate::monitor::hook::KeyboardHook::simulate_paste();
    log::info!("cmd::paste_entry: done");
    Ok(())
}

/// Write entry content to clipboard, hide jPaste, then simulate Ctrl+V.
///
/// The correct order is critical:
/// 1. Write to clipboard (while jPaste is still visible)
/// 2. Hide jPaste so focus returns to the previous application
/// 3. Wait briefly for the system to deliver focus
/// 4. Simulate Ctrl+V to paste into the previously focused app
#[tauri::command]
pub fn paste_entry_and_hide(
    state: State<'_, Arc<Mutex<AppState>>>,
    app: tauri::AppHandle,
    id: i64,
) -> Result<(), String> {
    log::info!("cmd::paste_entry_and_hide: id={}", id);

    // 1. Read content from DB
    let content = {
        let s = lock_state!(state);
        s.history.get_entry_content(id)?
    };
    log::info!("cmd::paste_entry_and_hide: got content ({} bytes)", content.len());

    // 2. Bump updated_at and increment copy_count
    {
        let s = lock_state!(state);
        s.history.touch_entry(id)?;
        let threshold = s.settings.get_settings()
            .map(|settings| if settings.auto_fav_on_copy_count { settings.auto_fav_threshold.max(2).min(10) as i64 } else { 0 })
            .unwrap_or(0);
        let _ = s.history.increment_copy_count(id, threshold);
    }

    // 3. Write to clipboard
    let mgr = {
        let s = lock_state!(state);
        s.clipboard_mgr.clone()
    };
    {
        let mut c = mgr.lock().map_err(|e| e.to_string())?;
        c.write_text(&content)?;
    }
    log::info!("cmd::paste_entry_and_hide: clipboard written");

    // 3.5 Notify frontend to refresh the list
    let _ = app.emit(EVENT_CLIPBOARD_UPDATED, serde_json::json!({}));

    // 3. Hide main window — focus returns to the app the user was editing in
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|e| format!("Failed to hide window: {}", e))?;
    }
    log::info!("cmd::paste_entry_and_hide: window hidden, waiting for focus handoff...");

    // 4. Wait for the system to deliver focus to the previously active window
    std::thread::sleep(std::time::Duration::from_millis(200));

    // 5. Simulate Ctrl+V — this now goes to the app the user was in
    log::info!("cmd::paste_entry_and_hide: simulating paste...");
    crate::monitor::hook::KeyboardHook::simulate_paste();
    log::info!("cmd::paste_entry_and_hide: done");

    Ok(())
}
