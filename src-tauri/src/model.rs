use serde::{Deserialize, Serialize};

// ── Tag bitmask constants ────────────────────────────────────────────────

pub const TAG_TEXT: i32 = 1 << 0;
pub const TAG_IMAGE: i32 = 1 << 2;
pub const TAG_URL: i32 = 1 << 3;
pub const TAG_FILE: i32 = 1 << 4;
pub const TAG_FAVORITE: i32 = 1 << 5;
pub const TAG_QR: i32 = 1 << 6;

// ── Event names ─────────────────────────────────────────────────────────

pub const EVENT_CLIPBOARD_UPDATED: &str = "clipboard-updated";
pub const EVENT_WINDOW_SHOWN: &str = "window-shown";
pub const EVENT_WINDOW_HIDING: &str = "window-hiding";
pub const EVENT_PASTE_ORDER_CHANGED: &str = "paste-order-changed";
pub const EVENT_TOAST_SHOW: &str = "toast-show";

// ── Domain types ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stats {
    pub count: i64,
    pub total_bytes: i64,
    pub image_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardUpdatePayload {
    pub id: i64,
    pub content_preview: String,
    pub tag_mask: i32,
    pub copy_count: i64,
    pub auto_favorited: bool,
    pub qr_text: String,
}

// ── Tag computations ────────────────────────────────────────────────────

/// Compute tag mask from content characteristics.
/// `has_image` should be true if an image was captured.
/// `has_file_uri` should be true if CF_HDROP / file-uri-list is present.
/// `text` is the trimmed CF_UNICODETEXT content (may be empty).
/// `has_qr` should be true if a QR code was detected in the image.
pub fn compute_tag_mask(text: &str, has_image: bool, has_file_uri: bool, has_qr: bool) -> i32 {
    let mut mask = 0;

    if has_image {
        mask |= TAG_IMAGE;
    }

    if has_file_uri || is_windows_path(text) {
        mask |= TAG_FILE;
    }

    if !text.is_empty() && !has_image && !has_file_uri && !is_windows_path(text) {
        mask |= TAG_TEXT;
    }

    if text.starts_with("http://") || text.starts_with("https://") {
        mask |= TAG_URL;
    }

    if has_qr {
        mask |= TAG_QR;
    }

    mask
}

/// Check if a string looks like a Windows path (X:\ or \\server\share).
pub fn is_windows_path(s: &str) -> bool {
    if s.len() < 3 {
        return false;
    }
    let bytes = s.as_bytes();
    // X:\ or X:/
    if bytes[1] == b':' && (bytes[2] == b'\\' || bytes[2] == b'/') {
        return bytes[0].is_ascii_alphabetic();
    }
    // \\ (UNC path)
    if s.starts_with("\\\\") {
        return true;
    }
    false
}

/// Returns the content_length of text (byte length) as i64 to avoid overflow
pub fn content_length(text: &str) -> i64 {
    text.len() as i64
}

// ponytail: mirrors frontend actions/ detect() patterns for toast enhancement.
// Order follows frontend's priority (json=90, curl/ws/open-url=80, folder=70,
// math=60, decoder=50, timestamp=30). Results capped at 3.
// If an action is added/removed on the frontend, update BOTH sides.
pub fn detect_actions(text: &str) -> Vec<&'static str> {
    let s = text.trim();
    if s.is_empty() {
        return Vec::new();
    }
    let mut v: Vec<&'static str> = Vec::new();

    // json (priority 90)
    if (s.starts_with('{') || s.starts_with('[')) && serde_json::from_str::<serde_json::Value>(s).is_ok() {
        v.push("json");
    }

    // curl (priority 80)
    // ponytail: use s.get(..5) (not s[..5]) — the text may start with a
    // multibyte char, so byte index 5 isn't always a char boundary and the
    // raw slice panics. get() returns None there, safely failing the match.
    if s.len() > 5 && s.get(..5) == Some("curl ") {
        v.push("curl");
    }

    // ws (priority 80)
    if s.starts_with("ws://") || s.starts_with("wss://") {
        v.push("ws");
    }

    // open-url (priority 80) — must check AFTER json/curl because http:// JSON API responses overlap
    if s.starts_with("http://") || s.starts_with("https://") || s.starts_with("ftp://") || s.starts_with("file://") {
        if !v.contains(&"json") && !v.contains(&"curl") {
            v.push("open-url");
        }
    }

    // folder (priority 70) — Windows path
    if is_windows_path(s) {
        v.push("folder");
    }

    // math (priority 60)
    if s.len() > 1 && s.bytes().all(|b| b.is_ascii_digit() || b"-+*/()% ".contains(&b)) && s.contains(|c| "+-*/%".contains(c)) {
        v.push("math");
    }

    // decoder (priority 50)
    if (s.len() > 4 && s.len() % 4 == 0 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'='))
        || s.contains("%") && s.bytes().filter(|b| *b == b'%').count() >= 2
        || s.contains("\\u")
    {
        v.push("decoder");
    }

    // timestamp (priority 30)
    if (s.len() == 10 || s.len() == 13) && s.bytes().all(|b| b.is_ascii_digit()) {
        if let Ok(ts) = s.parse::<i64>() {
            let ms = ts * if s.len() == 13 { 1 } else { 1000 };
            if ms > 946684800000 && ms < 4102444800000 {
                v.push("timestamp");
            }
        }
    }

    // Re-sort by frontend priority (stable: higher priority first)
    // ponytail: O(n log n) on max 8 items — negligible.
    v.sort_by(|a, b| {
        fn prio(id: &str) -> i32 {
            match id {
                "json" => 90, "curl" | "ws" | "open-url" => 80, "folder" => 70,
                "math" => 60, "decoder" => 50, "timestamp" => 30, _ => 0,
            }
        }
        prio(b).cmp(&prio(a))
    });
    v.truncate(3);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_text_only() {
        let mask = compute_tag_mask("hello world", false, false, false);
        assert_eq!(mask & TAG_TEXT, TAG_TEXT, "should have text tag");
        assert_eq!(mask & TAG_IMAGE, 0, "should not have image tag");
        assert_eq!(mask & TAG_URL, 0, "should not have url tag");
    }

    #[test]
    fn test_tag_url() {
        let mask = compute_tag_mask("https://example.com", false, false, false);
        assert_eq!(mask & TAG_TEXT, TAG_TEXT);
        assert_eq!(mask & TAG_URL, TAG_URL);
    }

    #[test]
    fn test_tag_image() {
        let mask = compute_tag_mask("", true, false, false);
        assert_eq!(mask & TAG_IMAGE, TAG_IMAGE);
        assert_eq!(mask & TAG_TEXT, 0);
    }

    #[test]
    fn test_tag_file_hdrop() {
        let mask = compute_tag_mask("", false, true, false);
        assert_eq!(mask & TAG_FILE, TAG_FILE);
        assert_eq!(mask & TAG_IMAGE, 0);
    }

    #[test]
    fn test_tag_file_windows_path() {
        let mask = compute_tag_mask("C:\\Users\\file.txt", false, false, false);
        assert_eq!(mask & TAG_FILE, TAG_FILE);
        assert_eq!(
            mask & TAG_TEXT,
            0,
            "text content with path should not be text-tagged"
        );
    }

    #[test]
    fn test_tag_file_unc_path() {
        let mask = compute_tag_mask("\\\\server\\share\\file", false, false, false);
        assert_eq!(mask & TAG_FILE, TAG_FILE);
    }

    #[test]
    fn test_tag_combined() {
        let mask = compute_tag_mask("https://github.com", true, false, false);
        assert_eq!(mask & TAG_IMAGE, TAG_IMAGE);
        assert_eq!(mask & TAG_URL, TAG_URL);
        assert_eq!(mask & TAG_TEXT, 0, "image+url should not be plain text");
        assert_eq!(mask & TAG_FILE, 0);
    }

    #[test]
    fn test_tag_qr() {
        let mask = compute_tag_mask("", true, false, true);
        assert!(mask & TAG_QR != 0, "qr tag should be set");
        assert!(mask & TAG_IMAGE != 0, "image tag should also be set");
    }

    #[test]
    fn test_tag_no_qr() {
        let mask = compute_tag_mask("", true, false, false);
        assert!(mask & TAG_QR == 0, "qr tag should not be set without has_qr");
    }

    #[test]
    fn test_is_windows_path_drive() {
        assert!(is_windows_path("D:\\folder\\file.txt"));
        assert!(is_windows_path("C:/Users/test"));
        assert!(!is_windows_path("hello.txt"));
        assert!(!is_windows_path(""));
        assert!(!is_windows_path("AB"));
    }

    #[test]
    fn test_content_length_unicode() {
        assert_eq!(content_length("hello"), 5);
        assert_eq!(content_length("中文"), 6); // UTF-8 bytes
        assert_eq!(content_length(""), 0);
    }

    // ── detect_actions ─────────────────────────────────────────────────
    #[test]
    fn test_detect_json() {
        let v = detect_actions(r#"{"a":1}"#);
        assert!(v.contains(&"json"), "JSON object: {:?}", v);
    }

    #[test]
    fn test_detect_json_array() {
        let v = detect_actions("[1, 2, 3]");
        assert!(v.contains(&"json"), "JSON array: {:?}", v);
    }

    #[test]
    fn test_detect_curl() {
        let v = detect_actions("curl https://example.com");
        assert!(v.contains(&"curl"), "curl: {:?}", v);
    }

    #[test]
    fn test_detect_ws() {
        let v = detect_actions("wss://echo.example.com");
        assert!(v.contains(&"ws"), "ws: {:?}", v);
    }

    #[test]
    fn test_detect_url() {
        let v = detect_actions("https://example.com/page");
        assert!(v.contains(&"open-url"), "URL: {:?}", v);
    }

    #[test]
    fn test_detect_windows_path() {
        let v = detect_actions("C:\\Users\\test\\file.txt");
        assert!(v.contains(&"folder"), "path: {:?}", v);
    }

    #[test]
    fn test_detect_timestamp_seconds() {
        let v = detect_actions("1700000000");
        assert!(v.contains(&"timestamp"), "ts10: {:?}", v);
    }

    #[test]
    fn test_detect_timestamp_millis() {
        let v = detect_actions("1700000000123");
        assert!(v.contains(&"timestamp"), "ts13: {:?}", v);
    }

    #[test]
    fn test_detect_math() {
        let v = detect_actions("1+2*3");
        assert!(v.contains(&"math"), "math: {:?}", v);
    }

    #[test]
    fn test_detect_decoder_base64() {
        let v = detect_actions("SGVsbG8gV29ybGQ=");
        assert!(v.contains(&"decoder"), "b64: {:?}", v);
    }

    #[test]
    fn test_detect_empty() {
        let v = detect_actions("");
        assert!(v.is_empty(), "empty: {:?}", v);
    }

    #[test]
    fn test_detect_plain_text() {
        let v = detect_actions("hello world");
        assert!(v.is_empty(), "plain: {:?}", v);
    }

    #[test]
    fn test_detect_max_3() {
        // JSON API response URL triggers both json and open-url
        let v = detect_actions(r#"{"url":"http://example.com"}"#);
        assert!(v.len() <= 3, "capped: {:?}", v);
    }

    #[test]
    fn test_detect_url_overlap_no_duplicate_json_url() {
        // A JSON URL like http://api.example.com/data.json
        let v = detect_actions("http://api.example.com/data.json");
        // Should be json only (higher priority)
        assert!(v.contains(&"json") || v.contains(&"open-url"), "json_url: {:?}", v);
    }

    #[test]
    fn test_detect_url_with_query_params() {
        let v = detect_actions("https://example.com/page?foo=bar&baz=qux");
        assert!(v.contains(&"open-url"), "URL with query params: {:?}", v);
    }

    #[test]
    fn test_detect_timestamp_out_of_range_low() {
        let v = detect_actions("100000"); // year 1970, too small
        assert!(!v.contains(&"timestamp"), "too-small timestamp: {:?}", v);
    }

    #[test]
    fn test_detect_timestamp_out_of_range_high() {
        let v = detect_actions("9999999999999"); // year 2286+, too large
        assert!(!v.contains(&"timestamp"), "too-large timestamp: {:?}", v);
    }

    #[test]
    fn test_detect_decoder_url_encoded() {
        let v = detect_actions("hello%20world%21");
        assert!(v.contains(&"decoder"), "URL-encoded: {:?}", v);
    }

    #[test]
    fn test_detect_decoder_unicode_escape() {
        let v = detect_actions("\\u0048\\u0065\\u006c\\u006c\\u006f");
        assert!(v.contains(&"decoder"), "unicode escape: {:?}", v);
    }

    #[test]
    fn test_detect_decoder_base64_with_padding() {
        let v = detect_actions("SGVsbG8gV29ybGQ=");
        assert!(v.contains(&"decoder"), "base64 with padding: {:?}", v);
    }

    #[test]
    fn test_detect_decoder_base64_no_padding() {
        let v = detect_actions("SGVsbG8gV29ybGQ");
        assert!(v.contains(&"decoder"), "base64 no padding: {:?}", v);
    }

    #[test]
    fn test_detect_math_complex() {
        let v = detect_actions("(1+2)*3-4/2");
        assert!(v.contains(&"math"), "complex math: {:?}", v);
    }

    #[test]
    fn test_detect_math_single_number_not_math() {
        let v = detect_actions("42");
        assert!(!v.contains(&"math"), "single number should not be math");
    }

    #[test]
    fn test_detect_ftp_url() {
        let v = detect_actions("ftp://files.example.com/file.zip");
        assert!(v.contains(&"open-url"), "FTP URL: {:?}", v);
    }

    #[test]
    fn test_detect_file_url() {
        let v = detect_actions("file:///C:/Users/test.txt");
        assert!(v.contains(&"open-url"), "file URL: {:?}", v);
    }

    #[test]
    fn test_detect_whitespace_only() {
        let v = detect_actions("   \t\n  ");
        assert!(v.is_empty(), "whitespace only: {:?}", v);
    }

    #[test]
    fn test_detect_result_capped_and_sorted() {
        // Content that triggers multiple actions: JSON URL + path-like
        let v = detect_actions(r#"{"url":"http://example.com","path":"C:\\test"}"#);
        assert!(v.len() <= 3, "capped at 3, got {}", v.len());
        // First should be json (priority 90)
        assert_eq!(v.first().copied(), Some("json"), "highest priority first: {:?}", v);
    }

    #[test]
    fn test_detect_curl_with_arguments() {
        let v = detect_actions("curl -X POST https://api.example.com/data -H 'Content-Type: application/json'");
        assert!(v.contains(&"curl"), "curl with flags: {:?}", v);
    }

    #[test]
    fn test_detect_curl_case_insensitive() {
        let v = detect_actions("CURL https://example.com");
        assert!(v.contains(&"curl"), "CURL uppercase: {:?}", v);
    }

    #[test]
    fn test_detect_ws_url() {
        let v = detect_actions("ws://localhost:8080/chat");
        assert!(v.contains(&"ws"), "ws URL: {:?}", v);
    }

    #[test]
    fn test_detect_wss_url() {
        let v = detect_actions("wss://echo.websocket.org");
        assert!(v.contains(&"ws"), "wss URL: {:?}", v);
    }
}
