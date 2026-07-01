//! Settings integration tests.
//!
//! Tests load/save/flush with real filesystem operations.

mod common;

use jpastev2::settings::{Config, SettingsService};

#[test]
fn test_settings_load_default_when_missing() {
    let dir = tempfile::tempdir().unwrap();
    let svc = SettingsService::load(&dir.path().to_path_buf()).unwrap();
    assert_eq!(svc.config().hotkey, "Alt+V");
    assert_eq!(svc.config().retain_days, 30);
}

#[test]
fn test_settings_modify_and_flush() {
    let dir = tempfile::tempdir().unwrap();
    let mut svc = SettingsService::load(&dir.path().to_path_buf()).unwrap();

    svc.config_mut().retain_days = 60;
    svc.config_mut().hotkey = "Ctrl+Shift+H".into();
    svc.flush().unwrap();

    // Re-load from same dir
    let svc2 = SettingsService::load(&dir.path().to_path_buf()).unwrap();
    assert_eq!(svc2.config().retain_days, 60);
    assert_eq!(svc2.config().hotkey, "Ctrl+Shift+H");
}

#[test]
fn test_settings_dirty_flag() {
    let dir = tempfile::tempdir().unwrap();
    let mut svc = SettingsService::load(&dir.path().to_path_buf()).unwrap();

    // First flush with no changes — file is NOT written (dirty=false)
    svc.flush().unwrap();
    let json_path = dir.path().join("settings.json");
    assert!(!json_path.exists(), "no file without changes");

    // Modify and flush — file should appear
    svc.config_mut().paste_order = "queue".into();
    svc.flush().unwrap();
    assert!(json_path.exists(), "file written after change");

    // Read back and verify
    let svc2 = SettingsService::load(&dir.path().to_path_buf()).unwrap();
    assert_eq!(svc2.config().paste_order, "queue");
}

#[test]
fn test_settings_config_roundtrip() {
    let config = Config::default();
    let json = serde_json::to_string_pretty(&config).unwrap();
    let parsed: Config = serde_json::from_str(&json).unwrap();
    assert_eq!(config.hotkey, parsed.hotkey);
    assert_eq!(config.retain_days, parsed.retain_days);
    assert_eq!(config.paste_order, parsed.paste_order);
    assert_eq!(config.sort_field, parsed.sort_field);
}
