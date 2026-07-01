use crate::storage::repository::Entry;
use crate::util::text::truncate_preview;

/// Maximum preview length for text display.
const PREVIEW_MAX: usize = 120;

/// Preview type for an entry.
pub enum EntryPreview {
    Text(String),
    Image,
    File(String),
    Empty,
}

/// Generate a human-readable preview for an entry.
pub fn preview_entry(entry: &Entry) -> EntryPreview {
    if entry.tag_mask & 4 != 0 {
        return EntryPreview::Image;
    }

    if entry.tag_mask & 16 != 0 {
        let preview = truncate_preview(&entry.content, PREVIEW_MAX);
        return EntryPreview::File(preview);
    }

    if !entry.content.is_empty() {
        let preview = truncate_preview(&entry.content, PREVIEW_MAX);
        EntryPreview::Text(preview)
    } else {
        EntryPreview::Empty
    }
}

/// Format the source label for display.
pub fn format_source(source_exe: &str) -> &str {
    if source_exe.is_empty() {
        return "未知";
    }
    // Extract filename from path
    source_exe
        .rsplit(&['/', '\\'][..])
        .next()
        .unwrap_or(source_exe)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::text::format_time;

    fn make_entry(text: &str, tag_mask: i32, content: &str) -> Entry {
        Entry {
            id: 1,
            content_hash: "h".into(),
            content: content.into(),
            source_exe: text.into(),
            source_title: "".into(),
            tag_mask,
            is_favorite: false,
            content_length: content.len() as i32,
            created_at: "2026-07-01T12:00:00.000".into(),
            updated_at: "2026-07-01T12:30:00.000".into(),
            image_path: None,
        }
    }

    #[test]
    fn test_preview_text() {
        let e = make_entry("app.exe", 1, "Hello World");
        match preview_entry(&e) {
            EntryPreview::Text(s) => assert_eq!(s, "Hello World"),
            _ => panic!("expected text"),
        }
    }

    #[test]
    fn test_preview_image() {
        let e = make_entry("app.exe", 4, "");
        match preview_entry(&e) {
            EntryPreview::Image => {} // ok
            _ => panic!("expected image"),
        }
    }

    #[test]
    fn test_format_source_filename() {
        assert_eq!(format_source(r"C:\Windows\notepad.exe"), "notepad.exe");
        assert_eq!(format_source(""), "未知");
    }

    #[test]
    fn test_format_time() {
        assert_eq!(format_time("2026-07-01T14:30:00.000"), "14:30:00");
    }
}
