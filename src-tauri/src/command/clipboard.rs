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

/// Write an entry's content to the clipboard.
/// Image entries write the actual image bytes (so copy/paste of image items is
/// meaningful); text entries write the text. Returns `true` when an image was
/// written.
fn write_entry_to_clipboard(
    state: &State<'_, Arc<Mutex<AppState>>>,
    id: i64,
) -> Result<bool, String> {
    // Read what we need from AppState first, releasing the lock before
    // touching the clipboard manager (lock order: state -> clipboard_mgr).
    let (img_path, text) = {
        let s = lock_state!(state);
        let img = s.history.get_entry_image_path(id).ok();
        let txt = if img.is_none() {
            s.history.get_entry_content(id).unwrap_or_default()
        } else {
            String::new()
        };
        (img, txt)
    };

    let mgr = {
        let s = lock_state!(state);
        s.clipboard_mgr.clone()
    };
    let mut c = mgr.lock().map_err(|e| e.to_string())?;
    match img_path {
        Some(path) => {
            let bytes = std::fs::read(&path).map_err(|e| format!("读取图片失败: {}", e))?;
            c.write_image(&bytes)?;
            Ok(true)
        }
        None => {
            c.write_text(&text)?;
            Ok(false)
        }
    }
}

/// Copy an entry to the clipboard (image bytes for image entries, text otherwise).
#[tauri::command]
pub fn copy_entry(
    state: State<'_, Arc<Mutex<AppState>>>,
    id: i64,
) -> Result<(), String> {
    write_entry_to_clipboard(&state, id).map(|_| ())
}

#[tauri::command]
pub fn paste_entry(
    state: State<'_, Arc<Mutex<AppState>>>,
    id: i64,
) -> Result<(), String> {
    log::info!("cmd::paste_entry: id={}", id);

    // Increment copy_count for user-triggered paste
    {
        let s = lock_state!(state);
        let threshold = s.settings.get_settings()
            .map(|settings| if settings.auto_fav_on_copy_count { settings.auto_fav_threshold.max(2).min(10) as i64 } else { 0 })
            .unwrap_or(0);
        let _ = s.history.increment_copy_count(id, threshold);
    }

    write_entry_to_clipboard(&state, id)?;
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

    // 1. Bump updated_at and increment copy_count
    {
        let s = lock_state!(state);
        s.history.touch_entry(id)?;
        let threshold = s.settings.get_settings()
            .map(|settings| if settings.auto_fav_on_copy_count { settings.auto_fav_threshold.max(2).min(10) as i64 } else { 0 })
            .unwrap_or(0);
        let _ = s.history.increment_copy_count(id, threshold);
    }

    // 2. Write to clipboard (image bytes for image entries, text otherwise)
    write_entry_to_clipboard(&state, id)?;
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
