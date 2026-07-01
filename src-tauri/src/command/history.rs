//! History-related Tauri commands

use crate::command::{lock_state, AppState};
use std::sync::{Arc, Mutex};
use tauri::State;

#[tauri::command]
pub fn get_entries(
    state: State<'_, Arc<Mutex<AppState>>>,
    search: String,
    tag_mask: i32,
    cursor_updated: i64,
    cursor_id: i64,
    limit: i32,
    sort_field: String,
    sort_order: String,
) -> Result<crate::service::history::QueryResult, String> {
    let s = lock_state!(state);
    let result = s.history
        .get_entries(&search, tag_mask, cursor_updated, cursor_id, limit, &sort_field, &sort_order)?;
    Ok(result)
}

#[tauri::command]
pub fn get_entry_content(
    state: State<'_, Arc<Mutex<AppState>>>,
    id: i64,
) -> Result<String, String> {
    let s = lock_state!(state);
    s.history.get_entry_content(id)
}

#[tauri::command]
pub fn get_entry_image(
    state: State<'_, Arc<Mutex<AppState>>>,
    id: i64,
) -> Result<String, String> {
    let s = lock_state!(state);
    s.history.get_entry_image_path(id)
}

#[tauri::command]
pub fn get_entry_image_full(
    state: State<'_, Arc<Mutex<AppState>>>,
    id: i64,
) -> Result<String, String> {
    let s = lock_state!(state);
    s.history.get_entry_image_full_path(id)
}

#[tauri::command]
pub fn delete_entry(
    state: State<'_, Arc<Mutex<AppState>>>,
    id: i64,
) -> Result<bool, String> {
    let s = lock_state!(state);
    let had_image = s.history.delete_entry(id)?;
    Ok(had_image)
}

#[tauri::command]
pub fn toggle_favorite(
    state: State<'_, Arc<Mutex<AppState>>>,
    id: i64,
    value: bool,
) -> Result<(), String> {
    let s = lock_state!(state);
    s.history.toggle_favorite(id, value)?;
    if !value {
        // If user manually unfavorites and auto-fav is enabled, mark as "never auto-fav again"
        if let Ok(settings) = s.settings.get_settings() {
            if settings.auto_fav_on_copy_count {
                let _ = s.history.set_no_auto_fav(id, true);
            }
        }
    } else {
        // Re-favoriting clears the no_auto_fav flag so auto-fav can trigger again
        let _ = s.history.set_no_auto_fav(id, false);
    }
    Ok(())
}

#[tauri::command]
pub fn cleanup(
    state: State<'_, Arc<Mutex<AppState>>>,
    retain_days: u32,
) -> Result<crate::service::history::CleanupResult, String> {
    let s = lock_state!(state);
    s.history.cleanup(retain_days)
}

#[tauri::command]
pub fn clear_all(
    state: State<'_, Arc<Mutex<AppState>>>,
    keep_favorites: bool,
) -> Result<(), String> {
    let s = lock_state!(state);
    s.history.clear_all(keep_favorites)
}

#[tauri::command]
pub fn get_stats(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<crate::service::history::StatsResponse, String> {
    let s = lock_state!(state);
    s.history.get_stats()
}

#[tauri::command]
pub fn get_entries_regex(
    state: State<'_, Arc<Mutex<AppState>>>,
    pattern: String,
    tag_mask: i32,
    sort_field: String,
    sort_order: String,
) -> Result<Vec<crate::service::history::EntryResponse>, String> {
    let s = lock_state!(state);
    s.history.get_entries_regex(&pattern, tag_mask, &sort_field, &sort_order)
}

#[tauri::command]
pub fn get_image_list(
    state: State<'_, Arc<Mutex<AppState>>>,
    tag_mask: i32,
    search: String,
) -> Result<Vec<i64>, String> {
    let s = lock_state!(state);
    s.history.get_image_list(tag_mask, &search)
}

#[tauri::command]
pub fn increment_copy_count(
    state: State<'_, Arc<Mutex<AppState>>>,
    id: i64,
) -> Result<(), String> {
    let s = lock_state!(state);
    let threshold = s.settings.get_settings()
        .map(|settings| if settings.auto_fav_on_copy_count { settings.auto_fav_threshold.max(2).min(10) as i64 } else { 0 })
        .unwrap_or(0);
    s.history.increment_copy_count(id, threshold)?;
    Ok(())
}

#[tauri::command]
pub fn scan_qr_text(
    state: State<'_, Arc<Mutex<AppState>>>,
    id: i64,
) -> Result<String, String> {
    let s = lock_state!(state);
    let qr = s.history.get_entry_qr_text(id)?;
    // If no qr_text stored, try on-demand decode from the image
    if qr.is_empty() {
        let img_path = s.history.get_entry_image_full_path(id)?;
        if let Ok(bytes) = std::fs::read(&img_path) {
            if let Some(decoded) = crate::qrcode::decode_qr_from_image(&bytes) {
                return Ok(decoded);
            }
        }
    }
    Ok(qr)
}
