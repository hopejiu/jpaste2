use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Tracks recently self-written clipboard content hashes.
///
/// When jPaste writes to the clipboard, it marks the written hash here
/// so that the clipboard-change callback can skip re-capturing its own output.
/// Entries expire after a configurable TTL (default 5 seconds).
pub struct SelfWriteTracker {
    entries: VecDeque<(String, Instant)>,
    ttl: Duration,
}

impl SelfWriteTracker {
    /// Create a new tracker with a 5-second TTL.
    pub fn new() -> Self {
        Self::with_ttl(Duration::from_secs(5))
    }

    /// Create a tracker with a custom TTL.
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            entries: VecDeque::new(),
            ttl,
        }
    }

    /// Mark a content hash as self-written.
    pub fn mark(&mut self, hash: String) {
        self.evict_expired();
        self.entries.push_back((hash, Instant::now()));
    }

    /// Check whether a hash was recently self-written (and thus should be skipped).
    pub fn is_self_write(&mut self, hash: &str) -> bool {
        self.evict_expired();
        self.entries
            .iter()
            .any(|(h, _)| h == hash)
    }

    /// Remove expired entries.
    fn evict_expired(&mut self) {
        let cutoff = Instant::now() - self.ttl;
        while let Some((_, time)) = self.entries.front() {
            if *time < cutoff {
                self.entries.pop_front();
            } else {
                break;
            }
        }
    }

    /// Clear all tracked entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Number of tracked entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

impl Default for SelfWriteTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_mark_and_check() {
        let mut t = SelfWriteTracker::new();
        t.mark("abc123".into());
        assert!(t.is_self_write("abc123"));
        assert!(!t.is_self_write("other"));
    }

    #[test]
    fn test_expiry() {
        let mut t = SelfWriteTracker::with_ttl(Duration::from_millis(10));
        t.mark("abc".into());
        assert!(t.is_self_write("abc"));
        std::thread::sleep(Duration::from_millis(20));
        assert!(!t.is_self_write("abc"));
    }

    #[test]
    fn test_clear() {
        let mut t = SelfWriteTracker::new();
        t.mark("a".into());
        t.mark("b".into());
        t.clear();
        assert_eq!(t.len(), 0);
        assert!(!t.is_self_write("a"));
    }

    #[test]
    fn test_multiple_entries() {
        let mut t = SelfWriteTracker::new();
        t.mark("x".into());
        t.mark("y".into());
        t.mark("z".into());
        assert!(t.is_self_write("y"));
        assert!(t.is_self_write("z"));
    }
}
