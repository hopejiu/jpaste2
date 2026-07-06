use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};

// ── Temp Directory ─────────────────────────────────────────────────────

/// Get the unified temp directory for jPaste (%TEMP%/jpaste2).
/// Creates it if it doesn't exist.
pub fn jpaste_temp_dir() -> std::path::PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push("jpaste2");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Clean up the entire jPaste temp directory.
/// Called on application exit.
pub fn cleanup_temp_dir() {
    let dir = jpaste_temp_dir();
    if dir.exists() {
        let _ = std::fs::remove_dir_all(&dir);
    }
}

// ── Hashing ─────────────────────────────────────────────────────────────

/// Compute SHA-256 hex string from text
pub fn sha256_hex(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Compute SHA-256 hex string from raw bytes
pub fn sha256_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

// ── Text truncation ─────────────────────────────────────────────────────

/// Truncate text to max_chars bytes, appending "..." if truncated
pub fn truncate(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let mut truncated = String::new();
    for c in text.chars() {
        let new_len = truncated.len() + c.len_utf8();
        if new_len > max_bytes.saturating_sub(3) {
            break;
        }
        truncated.push(c);
    }
    truncated.push_str("...");
    truncated
}

// ── Self-write tracker (anti-feedback loop) ────────────────────────────

/// Tracks clipboard writes made by jPaste itself, to avoid re-capturing
/// our own clipboard writes.
pub struct SelfWriteTracker {
    hash: String,
    content_len: usize,
    expires_at: Instant,
}

impl SelfWriteTracker {
    const TTL: Duration = Duration::from_millis(500);

    pub fn new() -> Self {
        Self {
            hash: String::new(),
            content_len: 0,
            expires_at: Instant::now(),
        }
    }

    /// Mark a text as self-written
    pub fn mark(&mut self, text: &str) {
        self.content_len = text.len();
        self.hash = sha256_hex(text);
        self.expires_at = Instant::now() + Self::TTL;
    }

    /// Check if the tracker has expired
    pub fn is_expired(&self) -> bool {
        Instant::now() > self.expires_at
    }

    /// Check if a given text matches our last self-write (within TTL).
    /// Fast path: compare byte length first to avoid hashing long content.
    pub fn is_self_write(&self, text: &str) -> bool {
        if self.is_expired() {
            return false;
        }
        // Length guard avoids SHA-256 on content that can't match.
        // Different content may share byte length, so full hash confirms.
        if text.len() != self.content_len {
            return false;
        }
        let h = sha256_hex(text);
        h == self.hash
    }

    /// Clear the tracker
    pub fn clear(&mut self) {
        self.hash.clear();
        self.content_len = 0;
        self.expires_at = Instant::now() - Duration::from_secs(1); // ensure expired
    }
}

impl Default for SelfWriteTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ── Timestamp helpers ─────────────────────────────────────────────────

/// Generate current Unix timestamp in milliseconds (UTC).
/// Precisely: milliseconds since Unix epoch, never localised.
pub fn chrono_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// Convert days since Unix epoch to (year, month, day)
pub fn days_to_date(days: i64) -> (i64, u32, u32) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d as u32)
}

// ── DIB → BMP header ──────────────────────────────────────────────────

/// Prepend a BMP file header to raw DIB (Device Independent Bitmap) data.
/// DIB is essentially a BMP without the 14-byte file header — this adds it.
#[allow(dead_code)]
pub fn prepend_bmp_header(dib: &[u8]) -> Vec<u8> {
    let file_size = dib.len() as u32 + 14;
    let mut bmp = Vec::with_capacity(file_size as usize);

    // BMP file header (14 bytes)
    bmp.push(b'B');
    bmp.push(b'M');
    bmp.extend_from_slice(&file_size.to_le_bytes());    // file size
    bmp.extend_from_slice(&[0u8; 4]);                    // reserved
    // pixel data offset: 14 (file header) + dib header size
    let dib_header_size = if dib.len() >= 4 {
        u32::from_le_bytes([dib[0], dib[1], dib[2], dib[3]])
    } else {
        40
    };
    let offset = 14u32 + dib_header_size;
    bmp.extend_from_slice(&offset.to_le_bytes());

    // Append the DIB data
    bmp.extend_from_slice(dib);

    bmp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_hex_consistency() {
        let h1 = sha256_hex("hello");
        let h2 = sha256_hex("hello");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }

    #[test]
    fn test_sha256_bytes_different_length() {
        let h = sha256_bytes(b"test data");
        assert_eq!(h.len(), 64);
    }

    #[test]
    fn test_truncate_short_text() {
        assert_eq!(truncate("hello", 100), "hello");
    }

    #[test]
    fn test_truncate_long_text() {
        let long = "abcdefghijklmnopqrstuvwxyz";
        let t = truncate(long, 10);
        assert!(t.len() <= 13); // 10 + "..."
        assert!(t.ends_with("..."));
    }

    #[test]
    fn test_truncate_unicode() {
        let s = "你好世界abcd";
        let t = truncate(s, 10);
        assert!(t.ends_with("..."));
    }

    #[test]
    fn test_truncate_empty() {
        assert_eq!(truncate("", 10), "");
    }

    #[test]
    fn test_self_write_tracker_mark_and_check() {
        let mut tracker = SelfWriteTracker::new();
        tracker.mark("my content");
        let h = sha256_hex("my content");
        assert_eq!(tracker.hash, h);
        assert!(tracker.is_self_write("my content"));
        assert!(!tracker.is_self_write("other content"));
    }

    #[test]
    fn test_self_write_tracker_expires() {
        let mut tracker = SelfWriteTracker::new();
        tracker.mark("test");
        // Force expiration by setting expires_at in the past
        tracker.expires_at = Instant::now() - Duration::from_secs(1);
        assert!(tracker.is_expired());
        assert!(!tracker.is_self_write("test"));
    }

    #[test]
    fn test_self_write_tracker_clear() {
        let mut tracker = SelfWriteTracker::new();
        tracker.mark("content");
        tracker.clear();
        assert!(tracker.is_expired());
        assert!(!tracker.is_self_write("content"));
    }

    #[test]
    fn test_prepend_bmp_header() {
        // Create a minimal DIB with 40-byte BITMAPINFOHEADER
        let mut dib = vec![0u8; 40];
        dib[0..4].copy_from_slice(&(40u32).to_le_bytes()); // header size
        dib.extend_from_slice(&[0u8; 100]); // pixel data

        let bmp = prepend_bmp_header(&dib);
        assert_eq!(bmp[0], b'B');
        assert_eq!(bmp[1], b'M');
        assert_eq!(bmp.len(), dib.len() + 14);
    }
}
