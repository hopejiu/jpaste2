//! Tauri command modules
//!
//! Each module groups related Tauri commands by domain.

pub mod clipboard;
pub mod curl;
pub mod filostack;
pub mod history;
pub mod settings;
pub mod system;
pub mod viewer;

use crate::service::history::HistoryService;
use crate::service::settings::SettingsService;
use crate::service::filostack::FiloStack;
use crate::hook::KeyboardHook;
use std::sync::{Arc, Mutex};

/// Shortcut for `state.lock().map_err(|e| e.to_string())?`
macro_rules! lock_state {
    ($state:expr) => {
        $state.lock().map_err(|e| e.to_string())?
    };
}
pub(crate) use lock_state;

/// Shared state for all Tauri commands
pub struct AppState {
    pub history: HistoryService,
    pub settings: SettingsService,
    pub filostack: FiloStack,
    pub clipboard_mgr: Arc<Mutex<crate::clipboard::ClipboardManager>>,
    pub app_handle: Option<tauri::AppHandle>,
    pub keyboard_hook: KeyboardHook,
    pub ctrl_v_sender: Mutex<Option<crate::service::filostack::CtrlVSender>>,
    pub pinned: Mutex<bool>,
}
