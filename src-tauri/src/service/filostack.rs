use serde::{Deserialize, Serialize};
use std::collections::LinkedList;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use crate::util::{truncate, SelfWriteTracker};

const MODE_NORMAL: &str = "normal";

/// Signal sent when Ctrl+V is intercepted (used by the queue-mode paste thread).
pub type CtrlVSender = Sender<()>;

/// FiloStack manages the paste queue for sequential (FIFO) pasting.
#[derive(Clone)]
pub struct FiloStack {
    items: Arc<Mutex<LinkedList<String>>>,
    mode: Arc<Mutex<String>>,
    self_tracker: Arc<Mutex<SelfWriteTracker>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FiloStatus {
    pub mode: String,
    pub mode_name: String,
    pub enabled: bool,
    pub item_count: usize,
    pub items: Vec<String>,
}

impl FiloStack {
    pub fn new() -> Self {
        Self::with_shared_tracker(Arc::new(Mutex::new(SelfWriteTracker::new())))
    }

    /// Create a new FiloStack with a shared self-write tracker.
    pub fn with_shared_tracker(tracker: Arc<Mutex<SelfWriteTracker>>) -> Self {
        Self {
            items: Arc::new(Mutex::new(LinkedList::new())),
            mode: Arc::new(Mutex::new(MODE_NORMAL.to_string())),
            self_tracker: tracker,
        }
    }

    /// Get current mode
    pub fn mode(&self) -> String {
        self.mode.lock().map(|m| m.clone()).unwrap_or(MODE_NORMAL.to_string())
    }

    /// Push text to the queue (only in non-normal mode, no self-writes)
    pub fn push(&self, text: &str) {
        if text.is_empty() {
            return;
        }
        let mode = self.mode.lock().unwrap().clone();
        if mode == MODE_NORMAL {
            return;
        }
        if let Ok(tracker) = self.self_tracker.lock() {
            if !tracker.is_expired() && tracker.is_self_write(text) {
                return;
            }
        }
        let mut items = self.items.lock().unwrap();
        items.push_back(text.to_string());
    }

    /// Pop the next item to paste (FIFO).
    pub fn pop(&self) -> Option<String> {
        let mut items = self.items.lock().unwrap();
        items.pop_front()
    }

    /// Display name for the active mode. Always "队列" since only FIFO is
    /// implemented; kept as a single source of truth.
    fn mode_name() -> &'static str {
        "队列"
    }

    /// Get current status for frontend display
    pub fn get_status(&self) -> FiloStatus {
        let items = self.items.lock().unwrap();
        let mode = self.mode.lock().unwrap().clone();
        let previews: Vec<String> = items.iter().map(|s| truncate(s, 40)).collect();
        FiloStatus {
            mode_name: Self::mode_name().to_string(),
            enabled: mode != MODE_NORMAL,
            item_count: items.len(),
            items: previews,
            mode,
        }
    }

    /// Set mode (normal / queue)
    pub fn set_mode(&self, mode: &str) {
        let old_mode = self.mode.lock().unwrap().clone();
        if old_mode == mode {
            return;
        }
        log::info!("filostack: mode change {} -> {}", old_mode, mode);

        {
            let mut current = self.mode.lock().unwrap();
            *current = mode.to_string();
        }

        self.clear();
    }

    /// Clear the queue
    pub fn clear(&self) {
        self.items.lock().unwrap().clear();
        if let Ok(mut tracker) = self.self_tracker.lock() {
            tracker.clear();
        }
    }

    /// Number of items in queue
    pub fn len(&self) -> usize {
        self.items.lock().unwrap().len()
    }

    /// Mark a text as self-written jPaste content
    pub fn mark_self_write(&self, text: &str) {
        if let Ok(mut tracker) = self.self_tracker.lock() {
            tracker.mark(text);
        }
    }
}

impl Default for FiloStack {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_stack() -> FiloStack {
        FiloStack::new()
    }

    #[test]
    fn test_normal_mode_ignores_push() {
        let stack = setup_stack();
        stack.push("test content");
        assert_eq!(stack.len(), 0, "normal mode should not push");
    }

    #[test]
    fn test_queue_mode_pushes_and_pops_fifo() {
        let stack = setup_stack();
        stack.set_mode("queue");
        stack.push("first");
        stack.push("second");
        stack.push("third");

        assert_eq!(stack.len(), 3);
        assert_eq!(stack.pop(), Some("first".to_string()));
        assert_eq!(stack.pop(), Some("second".to_string()));
        assert_eq!(stack.pop(), Some("third".to_string()));
        assert_eq!(stack.pop(), None);
    }

    #[test]
    fn test_queue_empty_pop() {
        let stack = setup_stack();
        stack.set_mode("queue");
        assert_eq!(stack.pop(), None);
    }

    #[test]
    fn test_set_mode_normal_clears_queue() {
        let stack = setup_stack();
        stack.set_mode("queue");
        stack.push("item1");
        stack.push("item2");
        assert_eq!(stack.len(), 2);

        stack.set_mode("normal");
        assert_eq!(stack.len(), 0);

        // Switching back should start fresh
        stack.set_mode("queue");
        assert_eq!(stack.len(), 0);
    }

    #[test]
    fn test_clear() {
        let stack = setup_stack();
        stack.set_mode("queue");
        stack.push("item1");
        stack.push("item2");
        stack.clear();
        assert_eq!(stack.len(), 0);
    }

    #[test]
    fn test_get_status() {
        let stack = setup_stack();
        stack.set_mode("queue");
        stack.push("hello world");

        let status = stack.get_status();
        assert_eq!(status.mode, "queue");
        assert!(status.enabled);
        assert_eq!(status.item_count, 1);
        assert!(!status.items.is_empty());
    }

    #[test]
    fn test_self_write_skip() {
        let stack = setup_stack();
        stack.set_mode("queue");
        stack.mark_self_write("my content");
        stack.push("my content");
        assert_eq!(stack.len(), 0, "self-write should be skipped");
    }

    #[test]
    fn test_push_after_self_write_expired() {
        let stack = setup_stack();
        stack.set_mode("queue");
        stack.mark_self_write("old content");
        // Force tracker to expire
        if let Ok(mut tracker) = stack.self_tracker.lock() {
            tracker.clear();
        }
        stack.push("old content");
        assert_eq!(stack.len(), 1, "after expiration, same text should be pushed");
    }

    #[test]
    fn test_empty_text_is_skipped() {
        let stack = setup_stack();
        stack.set_mode("queue");
        stack.push("");
        assert_eq!(stack.len(), 0);
    }

    #[test]
    fn test_mode_default_to_normal() {
        let stack = setup_stack();
        assert_eq!(stack.mode(), "normal");
    }
}
