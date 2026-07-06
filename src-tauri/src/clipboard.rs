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
}
