//! Clipboard module
//!
//! Wraps clipboard-rs for read/write operations and monitoring.

pub mod pipeline;

use clipboard_rs::{
    Clipboard, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext, ContentFormat,
};
use std::sync::Arc;
use std::sync::Mutex;

use crate::util::SelfWriteTracker;

/// Result of reading the clipboard.
/// Image data is stored as a temp file path (not in-memory Vec) to reduce peak memory.
#[derive(Debug, Default, Clone)]
pub struct ClipboardContent {
    pub text: String,
    pub has_image: bool,
    pub has_file_uri: bool,
    pub image_data: Option<Vec<u8>>,
    pub image_temp_path: Option<String>,
}

/// ClipboardManager wraps clipboard-rs for read/write operations.
pub struct ClipboardManager {
    ctx: clipboard_rs::ClipboardContext,
    pub self_tracker: Arc<Mutex<SelfWriteTracker>>,
}

impl ClipboardManager {
    /// Create a new ClipboardManager with a shared self-write tracker.
    pub fn with_shared_tracker(tracker: Arc<Mutex<SelfWriteTracker>>) -> Result<Self, String> {
        let ctx = clipboard_rs::ClipboardContext::new()
            .map_err(|e| format!("Failed to create clipboard context: {}", e))?;
        Ok(Self { ctx, self_tracker: tracker })
    }

    pub fn read(&mut self) -> ClipboardContent {
        let mut text = self.ctx.get_text().unwrap_or_default();
        let formats = self.ctx.available_formats().unwrap_or_default();
        let mut has_file_uri = formats.iter().any(|f| {
            f.contains("uri-list") || f.contains("HDROP") || f.contains("file")
        });

        // When files are copied, get_text() returns empty. Fetch file paths instead.
        if has_file_uri && text.is_empty() {
            if self.ctx.has(ContentFormat::Files) {
                if let Ok(files) = self.ctx.get_files() {
                    if !files.is_empty() {
                        text = files.join("\n");
                        log::info!("clipboard: captured {} file path(s)", files.len());
                    }
                }
            }
        }

        // ponytail: Some apps (e.g. VSCode) advertise custom formats containing
        // "file" (e.g. "code/file-list") but don't set CF_HDROP, so we can't
        // retrieve actual paths. If we still have empty text, reset the flag
        // so the upstream guard skips instead of saving a useless blank entry.
        if has_file_uri && text.is_empty() {
            has_file_uri = false;
            log::debug!("clipboard::read: has_file_uri reset to false — no readable file paths");
        }

        // Read image to temp file (avoids double get_image + peak memory for large screenshots)
        let (image_data, image_temp_path, has_image) = match self.read_image_to_temp_file() {
            Some(path) => {
                (None, Some(path), true)
            }
            None => {
                // Fallback: try reading into memory
                match self.read_image() {
                    Some(bytes) => (Some(bytes), None, true),
                    None => (None, None, false),
                }
            }
        };

        ClipboardContent { text, has_image, has_file_uri, image_data, image_temp_path }
    }

    pub fn read_image(&mut self) -> Option<Vec<u8>> {
        use clipboard_rs::common::RustImage;
        let img = self.ctx.get_image().ok()?;
        let png_buffer = img.to_png().ok()?;
        let bytes = png_buffer.get_bytes().to_vec();
        Some(bytes)
    }

    /// Read image directly to a temp file, returning the path.
    /// Avoids holding the full image in memory.
    /// Uses unified jpaste2 temp directory (cleaned up on app exit).
    fn read_image_to_temp_file(&mut self) -> Option<String> {
        use clipboard_rs::common::RustImage;
        let img = self.ctx.get_image().ok()?;
        let png_buffer = img.to_png().ok()?;
        let bytes = png_buffer.get_bytes();

        let temp_dir = crate::util::jpaste_temp_dir();
        let temp_path = temp_dir.join(format!("clip_{}.png", uuid::Uuid::new_v4()));
        if std::fs::write(&temp_path, bytes).is_ok() {
            Some(temp_path.to_string_lossy().to_string())
        } else {
            None
        }
    }

    pub fn write_text(&mut self, text: &str) -> Result<(), String> {
        self.ctx
            .set_text(text.to_string())
            .map_err(|e| format!("Failed to write text: {}", e))?;
        if let Ok(mut tracker) = self.self_tracker.lock() {
            tracker.mark(text);
        }
        Ok(())
    }

    /// Read the current clipboard text (empty string if none).
    pub fn get_text(&self) -> String {
        self.ctx.get_text().unwrap_or_default()
    }

    /// Write PNG image bytes to the clipboard, marking it as a self-write so
    /// the watcher doesn't re-capture our own generated image into history.
    pub fn write_image(&mut self, png_bytes: &[u8]) -> Result<(), String> {
        use clipboard_rs::common::{RustImage, RustImageData};
        let img = RustImageData::from_bytes(png_bytes)
            .map_err(|e| format!("Failed to decode image bytes: {}", e))?;
        // Mark BEFORE set_image so the suppression window covers the watcher fire.
        if let Ok(mut tracker) = self.self_tracker.lock() {
            tracker.mark_image();
        }
        self.ctx
            .set_image(img)
            .map_err(|e| format!("Failed to write image: {}", e))?;
        Ok(())
    }
}



// ── Clipboard Watcher ───────────────────────────────────────────────────

pub trait ClipboardEventHandler: Send + Sync + 'static {
    fn on_clipboard_change(&self, content: ClipboardContent);
}

pub fn start_watcher(
    manager: Arc<Mutex<ClipboardManager>>,
    handler: Arc<dyn ClipboardEventHandler>,
) -> Result<(), String> {
    std::thread::Builder::new()
        .name("clipboard-watcher".into())
        .spawn(move || {
            log::info!("clipboard: creating watcher on dedicated thread");

            let mut watcher = match ClipboardWatcherContext::new() {
                Ok(w) => w,
                Err(e) => {
                    log::error!("clipboard: failed to create watcher context: {}", e);
                    return;
                }
            };

            let impl_handler = ClipboardWatcherImpl {
                handler: handler.clone(),
                manager: manager.clone(),
            };

            let _handle = watcher.add_handler(impl_handler);
            log::info!("clipboard: handler registered, entering message loop");
            watcher.start_watch();
            log::warn!("clipboard: watcher message loop exited");
        })
        .map_err(|e| format!("Failed to spawn clipboard watcher thread: {}", e))?;

    log::info!("clipboard: watcher thread spawned");
    Ok(())
}

struct ClipboardWatcherImpl {
    handler: Arc<dyn ClipboardEventHandler>,
    manager: Arc<Mutex<ClipboardManager>>,
}

impl ClipboardHandler for ClipboardWatcherImpl {
    fn on_clipboard_change(&mut self) {
        match self.manager.lock() {
            Ok(mut mgr) => {
                if let Ok(text) = mgr.ctx.get_text() {
                    if let Ok(tracker) = mgr.self_tracker.lock() {
                        if !tracker.is_expired() && tracker.is_self_write(&text) {
                            return;
                        }
                    }
                }
                let content = mgr.read();
                // Skip images jPaste itself just wrote (QR/SVG export copy button).
                if content.has_image {
                    if let Ok(tracker) = mgr.self_tracker.lock() {
                        if tracker.is_image_self_write() {
                            return;
                        }
                    }
                }
                self.handler.on_clipboard_change(content);
            }
            Err(e) => log::error!("clipboard: lock manager: {}", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_ok() {
        let tracker = Arc::new(Mutex::new(SelfWriteTracker::new()));
        let mgr = ClipboardManager::with_shared_tracker(tracker);
        assert!(mgr.is_ok());
    }

    #[test]
    fn test_clipboard_content_default() {
        let content = ClipboardContent::default();
        assert!(content.text.is_empty(), "text should be empty");
        assert!(!content.has_image, "has_image should be false");
        assert!(!content.has_file_uri, "has_file_uri should be false");
        assert!(content.image_data.is_none(), "image_data should be none");
        assert!(content.image_temp_path.is_none(), "image_temp_path should be none");
    }

    #[test]
    fn test_clipboard_content_clone() {
        let content = ClipboardContent {
            text: "hello".to_string(),
            has_image: true,
            has_file_uri: false,
            image_data: Some(vec![1, 2, 3]),
            image_temp_path: Some("temp/path".to_string()),
        };
        let cloned = content.clone();
        assert_eq!(cloned.text, "hello");
        assert!(cloned.has_image);
        assert_eq!(cloned.image_data, Some(vec![1, 2, 3]));
        assert_eq!(cloned.image_temp_path, Some("temp/path".to_string()));
    }

    #[test]
    fn test_read_image_returns_none_when_no_image() {
        let tracker = Arc::new(Mutex::new(SelfWriteTracker::new()));
        let mut mgr = ClipboardManager::with_shared_tracker(tracker).unwrap();
        // No image on clipboard → should return None
        assert!(mgr.read_image().is_none());
    }
}
