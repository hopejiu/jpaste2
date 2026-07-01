use serde::{Deserialize, Serialize};

/// All persisted user settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Config {
    /// Global hotkey combo, e.g. "Alt+V".
    pub hotkey: String,
    /// Days to retain clipboard history before auto-cleanup.
    pub retain_days: u32,
    /// Start with Windows automatically.
    pub auto_start: bool,
    /// Start minimized to tray.
    pub start_minimized: bool,
    /// Show toast notification on new clipboard content.
    pub notify_enabled: bool,
    /// Toast opacity 0–100.
    pub notify_opacity: u32,
    /// Paste order: "normal" or "queue".
    pub paste_order: String,
    /// Window position: "center" or "remember".
    pub window_position: String,
    /// Sort field: "updated_at" or "content_length".
    pub sort_field: String,
    /// Sort direction: "asc" or "desc".
    pub sort_order: String,
    /// Auto-clear search on window show.
    pub auto_clear_search: bool,
    /// Seconds threshold for auto-clear (0 = always).
    pub auto_clear_seconds: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: "Alt+V".into(),
            retain_days: 30,
            auto_start: false,
            start_minimized: false,
            notify_enabled: false,
            notify_opacity: 100,
            paste_order: "normal".into(),
            window_position: "center".into(),
            sort_field: "updated_at".into(),
            sort_order: "desc".into(),
            auto_clear_search: false,
            auto_clear_seconds: 30,
        }
    }
}
