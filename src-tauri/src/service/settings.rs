use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchTargetKind {
    Web,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchTarget {
    pub id: String,
    pub name: String,
    pub kind: LaunchTargetKind,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hotkey: Option<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool { true }
fn default_true() -> bool { true }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Data {
    pub hotkey: String,
    pub retain_days: u32,
    pub auto_start: bool,
    pub start_minimized: bool,
    pub notify_enabled: bool,
    pub paste_order: String,      // "normal" / "queue"
    pub action_config: Value,     // frontend-managed, pass-through
    pub sort_field: String,       // "updated_at" / "content_length" / "copy_count"
    pub sort_order: String,       // "asc" / "desc"
    pub auto_clear_search: bool,
    pub auto_clear_seconds: u32,
    pub auto_hide_after_copy: bool,
    pub default_action: String,   // "copy" / "paste"
    pub center_on_show: bool,
    pub auto_fav_on_copy_count: bool,
    pub auto_fav_threshold: u32,
    /// When in queue mode, auto-switch back to normal mode once the queue empties/clears.
    #[serde(default = "default_true")]
    pub queue_auto_reset: bool,
    #[serde(default)]
    pub launch_targets: Vec<LaunchTarget>,
    /// Toolbox hotkeys: route (e.g. "/viewer/kankey") → shortcut string (e.g. "Alt+B").
    /// Empty map = no toolbox hotkeys configured (default).
    #[serde(default)]
    pub toolbox_hotkeys: HashMap<String, String>,
}

impl Default for Data {
    fn default() -> Self {
        Self {
            hotkey: "Alt+V".to_string(),
            retain_days: 30,
            auto_start: false,
            start_minimized: false,
            notify_enabled: false,
            paste_order: "normal".to_string(),
            action_config: serde_json::json!({}),
            sort_field: "updated_at".to_string(),
            sort_order: "desc".to_string(),
            auto_clear_search: false,
            auto_clear_seconds: 30,
            auto_hide_after_copy: false,
            default_action: "copy".to_string(),
            center_on_show: false,
            auto_fav_on_copy_count: false,
            auto_fav_threshold: 3,
            queue_auto_reset: true,
            launch_targets: Vec::new(),
            toolbox_hotkeys: HashMap::new(),
        }
    }
}

/// Callback type for settings change notifications.
/// Receives old and new Data so callers can diff whatever fields they care about.
pub type SettingsChangeCallback = Box<dyn Fn(&Data, &Data) -> Result<(), String> + Send>;

/// SettingsService manages settings.json file with read/write and change notifications
pub struct SettingsService {
    path: String,
    data: Mutex<Data>,
    on_settings_change: Mutex<Option<SettingsChangeCallback>>,
}

impl SettingsService {
    pub fn new(app_data: &Path) -> Self {
        Self {
            path: app_data.join("settings.json").to_string_lossy().to_string(),
            data: Mutex::new(Data::default()),
            on_settings_change: Mutex::new(None),
        }
    }

    /// Load settings from disk, falling back to defaults on error
    pub fn load(&self) -> Result<(), String> {
        match std::fs::read_to_string(&self.path) {
            Ok(content) => {
                let loaded: Data = serde_json::from_str(&content).map_err(|e| e.to_string())?;
                let mut data = self.data.lock().map_err(|e| e.to_string())?;
                *data = loaded;
                log::info!("settings: loaded from {:?}", self.path);
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                log::info!("settings: no file found at {:?}, using defaults", self.path);
                Ok(())
            }
            Err(e) => {
                log::warn!("settings: failed to load: {}", e);
                Err(e.to_string())
            }
        }
    }

    /// Save current settings to disk
    pub fn save(&self) -> Result<(), String> {
        let data = self.data.lock().map_err(|e| e.to_string())?;
        let content = serde_json::to_string_pretty(&*data).map_err(|e| e.to_string())?;
        // Ensure parent directory exists
        if let Some(parent) = std::path::Path::new(&self.path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&self.path, content).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Get current settings
    pub fn get_settings(&self) -> Result<Data, String> {
        self.data.lock().map(|d| d.clone()).map_err(|e| e.to_string())
    }

    /// Update settings with hotkey validation and change notifications
    pub fn save_settings(&self, new_data: Data) -> Result<(), String> {
        // Read old data without mutating
        let old_data = self.data.lock().map_err(|e| e.to_string())?.clone();

        // Fire callback BEFORE mutating so a failing callback (e.g. hotkey
        // registration error) prevents any state change.  The callback fires
        // when the main hotkey OR toolbox_hotkeys changed.
        let hotkey_changed = old_data.hotkey != new_data.hotkey
            || old_data.toolbox_hotkeys != new_data.toolbox_hotkeys;
        if hotkey_changed {
            if let Some(cb) = self.on_settings_change.lock().map_err(|e| e.to_string())?.as_ref() {
                cb(&old_data, &new_data)?;
            }
        }

        // Now safe to update in-memory data
        {
            let mut data = self.data.lock().map_err(|e| e.to_string())?;
            *data = new_data.clone();
        }

        // Persist
        self.save()
    }

    /// Update only launch_targets without triggering hotkey/settings callbacks.
    pub fn save_launch_targets(&self, targets: Vec<LaunchTarget>) -> Result<(), String> {
        {
            let mut data = self.data.lock().map_err(|e| e.to_string())?;
            data.launch_targets = targets;
        }
        self.save()
    }

    /// Return a snapshot of launch_targets.
    pub fn get_launch_targets(&self) -> Result<Vec<LaunchTarget>, String> {
        self.data
            .lock()
            .map(|d| d.launch_targets.clone())
            .map_err(|e| e.to_string())
    }

    /// Set settings change callback.  Receives (old_data, new_data) so callers
    /// can diff whatever fields they care about (hotkey, toolbox_hotkeys, etc.).
    pub fn on_settings_change<F>(&self, cb: F)
    where
        F: Fn(&Data, &Data) -> Result<(), String> + Send + 'static,
    {
        let mut callback = self.on_settings_change.lock().map_err(|_| ()).unwrap();
        *callback = Some(Box::new(cb));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_service() -> (SettingsService, TempDir) {
        let dir = TempDir::new().unwrap();
        let svc = SettingsService::new(dir.path());
        (svc, dir)
    }

    #[test]
    fn test_default_settings() {
        let (svc, _) = setup_service();
        let data = svc.get_settings().unwrap();
        assert_eq!(data.hotkey, "Alt+V");
        assert_eq!(data.retain_days, 30);
    }

    #[test]
    fn test_save_and_load() {
        let (svc, dir) = setup_service();
        let mut data = svc.get_settings().unwrap();
        data.hotkey = "Ctrl+Space".to_string();
        data.retain_days = 7;
        svc.save_settings(data.clone()).unwrap();

        // Create a new service pointing to the same file to verify persistence
        let svc2 = SettingsService::new(dir.path());
        svc2.load().unwrap();
        let loaded = svc2.get_settings().unwrap();
        assert_eq!(loaded.hotkey, "Ctrl+Space");
        assert_eq!(loaded.retain_days, 7);
    }

    #[test]
    fn test_hotkey_change_callback() {
        let (svc, _) = setup_service();
        let called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let called_clone = called.clone();

        svc.on_settings_change(move |old, new| {
            assert_eq!(old.hotkey, "Alt+V".to_string());
            assert_eq!(new.hotkey, "Ctrl+Q".to_string());
            called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        });

        let mut data = svc.get_settings().unwrap();
        data.hotkey = "Ctrl+Q".to_string();
        svc.save_settings(data).unwrap();

        assert!(called.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn test_toolbox_hotkeys_default_empty() {
        let (svc, _) = setup_service();
        let data = svc.get_settings().unwrap();
        assert!(data.toolbox_hotkeys.is_empty());
    }

    #[test]
    fn test_settings_change_callback_fires_on_toolbox_hotkey() {
        let (svc, _) = setup_service();
        let called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let called_clone = called.clone();

        svc.on_settings_change(move |_old, new| {
            assert_eq!(new.toolbox_hotkeys.get("/viewer/kanban"), Some(&"Alt+B".to_string()));
            called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        });

        let mut data = svc.get_settings().unwrap();
        data.toolbox_hotkeys.insert("/viewer/kanban".to_string(), "Alt+B".to_string());
        svc.save_settings(data).unwrap();

        assert!(called.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn test_load_nonexistent_file_uses_defaults() {
        let (svc, _) = setup_service();
        // load on a service without saving should use defaults
        assert!(svc.load().is_ok());
        let data = svc.get_settings().unwrap();
        assert_eq!(data.hotkey, "Alt+V");
    }
}
