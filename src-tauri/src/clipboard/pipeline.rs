//! Clipboard capture pipeline
//!
//! Handles the full flow: clipboard change → tag computation → dedup → store → notify.
//! Used by AppClipboardHandler in lib.rs.

use crate::model;
use std::sync::{Arc, Mutex};
use crate::command::AppState;

/// Clipboard pipeline that processes capture events.
/// Wraps AppState to provide a focused interface for clipboard handling.
pub struct ClipboardPipeline {
    pub(crate) state: Arc<Mutex<AppState>>,
}

impl ClipboardPipeline {
    pub fn new(state: Arc<Mutex<AppState>>) -> Self {
        Self { state }
    }

    pub fn process(
        &self,
        text: &str,
        has_image: bool,
        has_file_uri: bool,
        hash: &str,
        image_data: Option<&[u8]>,
        qr_text: &str,
    ) -> Result<model::ClipboardUpdatePayload, String> {
        let tag_mask = model::compute_tag_mask(text, has_image, has_file_uri, !qr_text.is_empty());

        let state = self.state.lock().map_err(|e| e.to_string())?;
        let mut payload = state.history.save_clipboard(hash, text, tag_mask, image_data, qr_text)?;

        // Auto-favorite on dedup when copy_count reaches the threshold
        // ponytail: uses shared HistoryService::try_auto_favorite_by_id so
        // clipboard-capture and user-triggered paste paths behave identically.
        if payload.copy_count > 0 {
            if let Ok(settings) = state.settings.get_settings() {
                if settings.auto_fav_on_copy_count {
                    let threshold = (settings.auto_fav_threshold.max(2).min(10)) as i64;
                    if state.history.try_auto_favorite_by_id(payload.id, payload.copy_count, threshold).unwrap_or(false) {
                        payload.auto_favorited = true;
                    }
                }
            }
        }

        Ok(payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::history::HistoryService;
    use crate::service::settings::SettingsService;
    use crate::service::filostack::FiloStack;
    use crate::util::SelfWriteTracker;
    use tempfile::TempDir;

    fn setup_pipeline() -> (ClipboardPipeline, TempDir) {
        let dir = TempDir::new().unwrap();
        let path = dir.path();
        let history = HistoryService::new(path).unwrap();
        let settings = SettingsService::new(path);
        let tracker = Arc::new(Mutex::new(SelfWriteTracker::new()));
        let filostack = FiloStack::with_shared_tracker(tracker.clone());
        let state = Arc::new(Mutex::new(AppState {
            history,
            settings,
            filostack,
            clipboard_mgr: Arc::new(Mutex::new(
                crate::clipboard::ClipboardManager::with_shared_tracker(tracker.clone()).unwrap(),
            )),
            app_handle: None,
            keyboard_hook: crate::hook::KeyboardHook::new(),
            ctrl_v_sender: Mutex::new(None),
            pinned: Mutex::new(false),
            launch_hotkey_map: std::sync::Mutex::new(std::collections::HashMap::new()),
        }));
        let pipeline = ClipboardPipeline::new(state);
        (pipeline, dir)
    }

    #[test]
    fn test_process_text_entry() {
        let (pipeline, _dir) = setup_pipeline();
        let hash = crate::util::sha256_hex("hello world");
        let result = pipeline.process("hello world", false, false, &hash, None, "").unwrap();
        assert!(result.id > 0);
    }

    #[test]
    fn test_process_with_image() {
        let (pipeline, _dir) = setup_pipeline();
        let fake_png = vec![0x89, 0x50, 0x4E, 0x47];
        let hash = crate::util::sha256_bytes(&fake_png);
        let result = pipeline.process("", true, false, &hash, Some(&fake_png), "").unwrap();
        assert!(result.id > 0);
    }

    #[test]
    fn test_process_with_qr_text() {
        let (pipeline, _dir) = setup_pipeline();
        let fake_png = vec![0x89, 0x50, 0x4E, 0x47];
        let hash = crate::util::sha256_bytes(&fake_png);
        let result = pipeline.process("", true, false, &hash, Some(&fake_png), "https://qr.example.com").unwrap();
        assert!(result.id > 0);
        assert_eq!(result.qr_text, "https://qr.example.com");
    }

    #[test]
    fn test_process_dedup_increments_copy_count() {
        let (pipeline, _dir) = setup_pipeline();
        let hash = crate::util::sha256_hex("dedup content");
        let r1 = pipeline.process("dedup content", false, false, &hash, None, "").unwrap();
        // Second save with same hash — dedup
        let r2 = pipeline.process("dedup content updated", false, false, &hash, None, "").unwrap();
        assert_eq!(r1.id, r2.id, "same hash should reuse entry id");
        assert_eq!(r2.copy_count, 1, "dedup should increment copy_count to 1");
    }

    #[test]
    fn test_process_auto_favorite_on_dedup_threshold() {
        let (pipeline, _dir) = setup_pipeline();
        // Enable auto_fav_on_copy_count with threshold 2
        {
            let state = pipeline.state.lock().unwrap();
            let mut settings = state.settings.get_settings().unwrap();
            settings.auto_fav_on_copy_count = true;
            settings.auto_fav_threshold = 2;
            state.settings.save_settings(settings).unwrap();
        }

        let hash = crate::util::sha256_hex("auto fav content");
        // Insert → copy_count=0, no auto-fav
        let r1 = pipeline.process("auto fav content", false, false, &hash, None, "").unwrap();
        assert!(!r1.auto_favorited, "insert should not auto-fav");

        // 1st dedup → copy_count=1, still below threshold 2
        let r2 = pipeline.process("auto fav v2", false, false, &hash, None, "").unwrap();
        assert!(!r2.auto_favorited, "copy_count=1 < threshold=2");

        // 2nd dedup → copy_count=2, hits threshold 2
        let r3 = pipeline.process("auto fav v3", false, false, &hash, None, "").unwrap();
        assert!(r3.auto_favorited, "copy_count=2 >= threshold=2 should trigger auto-fav");
    }

    #[test]
    fn test_process_file_uri_entry() {
        let (pipeline, _dir) = setup_pipeline();
        let hash = crate::util::sha256_hex("C:\\Users\\file.txt");
        let result = pipeline.process("C:\\Users\\file.txt", false, true, &hash, None, "").unwrap();
        assert!(result.id > 0, "file URI entry should be saved");
    }



}
