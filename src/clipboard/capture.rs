//! Clipboard capture logic using clipboard-rs.

use crate::util::hash::{sha256_hex, sha256_hex_bytes};
use clipboard_rs::{Clipboard, ContentFormat};
use clipboard_rs::common::RustImage;

pub mod tag {
    pub const TEXT: i32 = 1;
    pub const IMAGE: i32 = 4;
    pub const URL: i32 = 8;
    pub const FILE: i32 = 16;
    pub const FAVORITE: i32 = 32;
}

#[derive(Debug, Clone)]
pub struct CapturedData {
    pub primary_text: Option<String>,
    /// PNG-encoded image bytes.
    pub image_png: Option<Vec<u8>>,
    pub file_paths: Vec<String>,
    pub content_hash: String,
    pub tag_mask: i32,
    pub content_length: i32,
}

/// Capture current clipboard content.
pub fn capture_current(clip: &impl Clipboard) -> Option<CapturedData> {
    let has_text = clip.has(ContentFormat::Text);
    let has_image = clip.has(ContentFormat::Image);
    let has_files = clip.has(ContentFormat::Files);

    if !has_text && !has_image && !has_files {
        return None;
    }

    let primary_text = if has_text {
        clip.get_text().ok().filter(|s| !s.is_empty())
    } else {
        None
    };

    let image_png = if has_image {
        clip.get_image().ok().and_then(|img| encode_image_to_png(&img))
    } else {
        None
    };

    let file_paths = if has_files {
        clip.get_files().ok().unwrap_or_default()
    } else {
        vec![]
    };

    let content_hash = compute_hash(&primary_text, &image_png, &file_paths);
    let tag_mask = compute_tag_mask(&primary_text, has_image, &file_paths);
    let content_length = primary_text.as_ref().map(|s| s.len() as i32).unwrap_or(0);

    Some(CapturedData {
        primary_text,
        image_png,
        file_paths,
        content_hash,
        tag_mask,
        content_length,
    })
}

/// Encode an image to PNG bytes using the `image` crate.
fn encode_image_to_png(img: &clipboard_rs::RustImageData) -> Option<Vec<u8>> {
    let buf = img.to_png().ok()?;
    Some(buf.get_bytes().to_vec())
}

fn compute_hash(text: &Option<String>, image: &Option<Vec<u8>>, files: &[String]) -> String {
    if let Some(t) = text {
        sha256_hex(t.trim())
    } else if let Some(img) = image {
        sha256_hex_bytes(img)
    } else if !files.is_empty() {
        let joined = files.join("|");
        sha256_hex(&joined)
    } else {
        sha256_hex_bytes(b"empty")
    }
}

fn compute_tag_mask(text: &Option<String>, has_image: bool, files: &[String]) -> i32 {
    let mut mask = 0;
    if has_image {
        mask |= tag::IMAGE;
    }
    if !files.is_empty() {
        mask |= tag::FILE;
    }
    if let Some(t) = text {
        let trimmed = t.trim();
        if !trimmed.is_empty() {
            if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                mask |= tag::URL;
            }
            if is_windows_path(trimmed) {
                mask |= tag::FILE;
            }
            if mask == 0 {
                mask |= tag::TEXT;
            }
        }
    }
    mask
}

fn is_windows_path(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() >= 3 && bytes[1] == b':' && bytes[2] == b'\\' {
        let c = bytes[0];
        return (c >= b'A' && c <= b'Z') || (c >= b'a' && c <= b'z');
    }
    if bytes.len() >= 2 && bytes[0] == b'\\' && bytes[1] == b'\\' {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_windows_path() {
        assert!(is_windows_path(r"C:\Users"));
        assert!(is_windows_path(r"D:\"));
        assert!(is_windows_path(r"\\server\share"));
        assert!(!is_windows_path("hello"));
        assert!(!is_windows_path("http://example.com"));
    }

    #[test]
    fn test_compute_tag_mask_text_only() {
        let mask = compute_tag_mask(&Some("hello".into()), false, &[]);
        assert_eq!(mask, tag::TEXT);
    }

    #[test]
    fn test_compute_tag_mask_url() {
        let mask = compute_tag_mask(&Some("https://example.com".into()), false, &[]);
        assert_eq!(mask, tag::URL);
    }

    #[test]
    fn test_compute_tag_mask_image() {
        let mask = compute_tag_mask(&None, true, &[]);
        assert_eq!(mask, tag::IMAGE);
    }

    #[test]
    fn test_compute_primary_hash_consistency() {
        let h1 = compute_hash(&Some("hello".into()), &None, &[]);
        let h2 = compute_hash(&Some("hello".into()), &None, &[]);
        assert_eq!(h1, h2);
    }
}
