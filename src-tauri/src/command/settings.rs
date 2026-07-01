//! Settings-related Tauri commands

use crate::command::{lock_state, AppState};
use std::sync::{Arc, Mutex};
use tauri::State;

#[tauri::command]
pub fn get_settings(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<crate::service::settings::Data, String> {
    let s = lock_state!(state);
    s.settings.get_settings()
}

#[tauri::command]
pub fn save_settings(
    state: State<'_, Arc<Mutex<AppState>>>,
    data: crate::service::settings::Data,
) -> Result<(), String> {
    let s = lock_state!(state);
    s.settings.save_settings(data)
}
