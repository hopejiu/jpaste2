/// Truncate text to a maximum byte length, appending "…" if truncated.
/// Operates on byte length (for DB preview compatibility).
pub fn truncate_preview(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    // Ensure we don't split in the middle of a multi-byte char.
    let mut end = max_bytes.saturating_sub(3); // room for "…"
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut result: String = text[..end].into();
    result.push('…');
    result
}

/// Format ISO timestamp for display (showing only time portion).
/// Input format: "2026-07-01T14:30:00.000" → "14:30:00"
pub fn format_time(updated_at: &str) -> &str {
    if let Some(t) = updated_at.split('T').nth(1) {
        if let Some(ms) = t.split_once('.') {
            return ms.0;
        }
        return t;
    }
    updated_at
}

/// Try to parse an integer from a string without allocating.
pub fn format_int(n: i64) -> String {
    n.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_short_text() {
        assert_eq!(truncate_preview("hi", 10), "hi");
    }

    #[test]
    fn test_truncate_exact_fit() {
        assert_eq!(truncate_preview("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_long_text() {
        let long = "a".repeat(100);
        let result = truncate_preview(&long, 10);
        assert!(result.len() <= 10);
        assert!(result.ends_with('…'));
    }

    #[test]
    fn test_truncate_multibyte_char_boundary() {
        let text = "你好世界"; // 12 bytes, 4 chars
        let result = truncate_preview(text, 5);
        // 5 bytes: "你" is 3 bytes, "好" is 3 bytes → won't fit → "…"
        assert_eq!(result, "…");
    }

    #[test]
    fn test_format_int() {
        assert_eq!(format_int(0), "0");
        assert_eq!(format_int(42), "42");
        assert_eq!(format_int(-1), "-1");
        assert_eq!(format_int(i64::MAX), "9223372036854775807");
    }
}
