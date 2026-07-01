mod types;
pub use types::*;

use anyhow::Result;
use std::path::PathBuf;

/// Service managing settings.json I/O.
pub struct SettingsService {
    path: PathBuf,
    config: Config,
    dirty: bool,
}

impl SettingsService {
    /// Load settings from `{data_dir}/settings.json`.
    pub fn load(data_dir: &PathBuf) -> Result<Self> {
        let path = data_dir.join("settings.json");
        let config = if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Config::default()
        };
        Ok(Self { path, config, dirty: false })
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut Config {
        self.dirty = true;
        &mut self.config
    }

    /// Persist to disk if dirty, then reset dirty flag.
    pub fn flush(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }
        let json = serde_json::to_string_pretty(&self.config)?;
        std::fs::write(&self.path, json)?;
        self.dirty = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_default() {
        let c = Config::default();
        assert_eq!(c.hotkey, "Alt+V");
        assert_eq!(c.retain_days, 30);
        assert!(!c.auto_start);
        assert_eq!(c.paste_order, "normal");
        assert_eq!(c.sort_field, "updated_at");
        assert_eq!(c.window_position, "center");
    }

    #[test]
    fn test_serde_roundtrip() {
        let c = Config::default();
        let json = serde_json::to_string_pretty(&c).unwrap();
        let c2: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(c.hotkey, c2.hotkey);
        assert_eq!(c.retain_days, c2.retain_days);
    }
}
