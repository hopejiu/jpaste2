//! FiloStack service — manages the clipboard paste queue.
//!
//! In "queue" mode, copied items are pushed onto a FIFO queue.
//! A WH_KEYBOARD_LL hook intercepts Ctrl+V, pops from the queue,
//! writes to the clipboard, and lets the keystroke pass through.

use crate::filostack::strategy::{PasteStrategy, QueueStrategy};
use crate::util::tracker::SelfWriteTracker;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Callback type for writing text to the system clipboard.
pub type WriteTextFn = Box<dyn Fn(&str) + Send>;

/// Callback type for showing a toast notification.
pub type NotifyFn = Box<dyn Fn(&str, &str) + Send>;

/// FiloStack service state.
pub struct FiloStackService {
    /// Current mode: "normal" or "queue".
    mode: String,
    /// The FIFO queue of text items.
    queue: VecDeque<String>,
    /// Current paste strategy.
    strategy: Box<dyn PasteStrategy + Send>,
    /// Function to write text to clipboard.
    write_text: Option<WriteTextFn>,
    /// Function to show notification.
    notify: Option<NotifyFn>,
    /// Self-write tracker to avoid re-capturing own writes.
    tracker: SelfWriteTracker,
    /// Timestamp until which Ctrl+V should NOT be intercepted (self-paste guard).
    self_paste_until: Option<Instant>,
    /// Window for self-paste guard (milliseconds).
    self_paste_window: Duration,
}

impl FiloStackService {
    pub fn new() -> Self {
        Self {
            mode: "normal".into(),
            queue: VecDeque::new(),
            strategy: Box::new(QueueStrategy),
            write_text: None,
            notify: None,
            tracker: SelfWriteTracker::new(),
            self_paste_until: None,
            self_paste_window: Duration::from_millis(500),
        }
    }

    pub fn with_write_text(mut self, f: WriteTextFn) -> Self {
        self.write_text = Some(f);
        self
    }

    pub fn with_notify(mut self, f: NotifyFn) -> Self {
        self.notify = Some(f);
        self
    }

    pub fn mode(&self) -> &str {
        &self.mode
    }

    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    pub fn queue_items(&self) -> Vec<String> {
        self.queue.iter().cloned().collect()
    }

    /// Set mode: "normal" or "queue". Clears queue on mode change.
    pub fn set_mode(&mut self, mode: &str) {
        if self.mode != mode {
            self.mode = mode.to_string();
            self.queue.clear();
        }
    }

    /// Push a text item onto the queue (when queue mode is active).
    /// Returns true if item was queued.
    pub fn push(&mut self, text: &str) -> bool {
        if self.mode != "queue" {
            return false;
        }
        // Skip self-writes
        let hash = crate::util::hash::sha256_hex(text.trim());
        if self.tracker.is_self_write(&hash) {
            return false;
        }
        self.queue.push_back(text.to_string());
        true
    }

    /// Pop the next item from the queue (called by keyboard hook on Ctrl+V).
    pub fn pop(&mut self) -> Option<String> {
        if self.mode != "queue" {
            return None;
        }
        if self.self_paste_until.map_or(false, |t| Instant::now() < t) {
            // Self-paste guard active — don't intercept
            return None;
        }
        self.strategy.pop(&mut self.queue)
    }

    /// Mark the current write as self-paste (guard against re-capture).
    pub fn mark_self_paste(&mut self) {
        self.self_paste_until = Some(Instant::now() + self.self_paste_window);
    }

    pub fn mark_self_write(&mut self, hash: &str) {
        self.tracker.mark(hash.to_string());
    }

    /// Handle a keyboard hook trigger (Ctrl+V intercepted).
    /// Returns true if the key should be suppressed (hook consumed the event).
    pub fn handle_ctrl_v(&mut self) -> bool {
        if self.mode != "queue" {
            return false;
        }
        if self.self_paste_until.map_or(false, |t| Instant::now() < t) {
            return false; // self-paste guard, let it through
        }

        match self.pop() {
            Some(text) => {
                // Mark as self-write so we don't re-capture
                let hash = crate::util::hash::sha256_hex(text.trim());
                self.tracker.mark(hash);

                // Write to clipboard
                if let Some(ref write) = self.write_text {
                    write(&text);
                }

                // Notify
                if let Some(ref notify) = self.notify {
                    let msg = format!("已粘贴, 当前队列还有: {} 个", self.queue.len());
                    notify("jPaste", &msg);
                }

                true // hook consumed
            }
            None => false, // queue empty, let Ctrl+V pass through
        }
    }

    /// Auto-exit queue mode when non-text content is captured.
    pub fn auto_exit_if_needed(&mut self, tag_mask: i32) {
        if self.mode == "queue" && tag_mask & (crate::clipboard::capture::tag::IMAGE | crate::clipboard::capture::tag::FILE) != 0 {
            self.mode = "normal".to_string();
            self.queue.clear();
            if let Some(ref notify) = self.notify {
                notify("jPaste", "检测到图片/文件复制, 已自动退出队列模式");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_pop_fifo() {
        let mut svc = FiloStackService::new();
        svc.set_mode("queue");

        assert!(svc.push("a"));
        assert!(svc.push("b"));
        assert!(svc.push("c"));
        assert_eq!(svc.queue_len(), 3);

        assert_eq!(svc.pop(), Some("a".into()));
        assert_eq!(svc.pop(), Some("b".into()));
        assert_eq!(svc.pop(), Some("c".into()));
        assert_eq!(svc.pop(), None);
    }

    #[test]
    fn test_normal_mode_no_queue() {
        let mut svc = FiloStackService::new();
        // default is "normal"
        assert!(!svc.push("a"));
        assert_eq!(svc.pop(), None);
    }

    #[test]
    fn test_self_write_skipped() {
        let mut svc = FiloStackService::new();
        svc.set_mode("queue");

        let text = "hello";
        let hash = crate::util::hash::sha256_hex(text.trim());
        svc.mark_self_write(&hash);

        assert!(!svc.push(text)); // self-write should be skipped
        assert_eq!(svc.queue_len(), 0);
    }

    #[test]
    fn test_self_paste_guard() {
        let mut svc = FiloStackService::new();
        svc.set_mode("queue");
        svc.push("a");
        svc.mark_self_paste();
        // During self-paste guard, pop should return None
        assert_eq!(svc.pop(), None);
    }

    #[test]
    fn test_auto_exit_on_image() {
        let mut svc = FiloStackService::new();
        svc.set_mode("queue");
        svc.push("a");
        svc.auto_exit_if_needed(crate::clipboard::capture::tag::IMAGE);
        assert_eq!(svc.mode(), "normal");
        assert_eq!(svc.queue_len(), 0);
    }

    #[test]
    fn test_auto_exit_on_file() {
        let mut svc = FiloStackService::new();
        svc.set_mode("queue");
        svc.push("a");
        svc.auto_exit_if_needed(crate::clipboard::capture::tag::FILE);
        assert_eq!(svc.mode(), "normal");
    }

    #[test]
    fn test_text_only_does_not_exit() {
        let mut svc = FiloStackService::new();
        svc.set_mode("queue");
        svc.push("a");
        svc.auto_exit_if_needed(crate::clipboard::capture::tag::TEXT);
        assert_eq!(svc.mode(), "queue");
    }
}
