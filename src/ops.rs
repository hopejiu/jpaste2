//! Entry operations — thin functions that combine Repository + side effects.
//!
//! Extracted from `app.rs` so each operation can be unit-tested without
//! instantiating the full App struct. App methods become one-liner delegations.

use anyhow::Result;

use crate::storage::repository::{Entry, Repository};
use crate::util::hash::sha256_hex;
use crate::util::tracker::SelfWriteTracker;

/// Copy entry text to the system clipboard.
/// Returns the text content on success.
pub fn copy_entry(
    repo: &Repository,
    tracker: &mut SelfWriteTracker,
    entry_id: i64,
) -> Result<Option<String>> {
    let Some(text) = repo.get_format_content(entry_id, 13)? else {
        return Ok(None);
    };
    let hash = sha256_hex(text.trim());
    tracker.mark(hash);

    let clip = clipboard_rs::ClipboardContext::new()
        .map_err(|e| anyhow::anyhow!("clipboard: {}", e))?;
    use clipboard_rs::Clipboard;
    clip.set_text(text.clone())
        .map_err(|e| anyhow::anyhow!("clipboard set_text: {}", e))?;
    Ok(Some(text))
}

/// Delete an entry and remove it from the in-memory list.
pub fn delete_entry(
    repo: &Repository,
    entries: &mut Vec<Entry>,
    index: usize,
    selected_index: &mut usize,
) -> Result<()> {
    if index >= entries.len() {
        return Ok(());
    }
    repo.delete_entry(entries[index].id)?;
    entries.remove(index);
    if *selected_index >= entries.len() && !entries.is_empty() {
        *selected_index = entries.len() - 1;
    }
    Ok(())
}

/// Toggle the favorite flag on an entry.
pub fn toggle_favorite(repo: &Repository, entry: &mut Entry) -> Result<bool> {
    let fav = repo.toggle_favorite(entry.id)?;
    entry.is_favorite = fav;
    Ok(fav)
}

/// Open entry content in the default text editor.
pub fn open_in_editor(repo: &Repository, entry_id: i64) -> Result<()> {
    use std::io::Write;

    let Some(text) = repo.get_format_content(entry_id, 13)? else {
        return Ok(());
    };
    let p = std::env::temp_dir().join(format!("jpaste_{}.txt", entry_id));
    let mut f = std::fs::File::create(&p)?;
    f.write_all(text.as_bytes())?;

    if std::process::Command::new("code").arg(&p).spawn().is_err() {
        let _ = open::that(&p);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db::{self, DbConnection};
    use std::sync::Mutex;

    fn setup_repo() -> Repository {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        db::migrate(&conn).unwrap();
        Repository::new(DbConnection {
            conn: Mutex::new(conn),
        })
    }

    fn make_entry(id: i64, text: &str) -> Entry {
        Entry {
            id,
            content_hash: format!("h{}", id),
            content: text.into(),
            source_exe: "test.exe".into(),
            source_title: "".into(),
            tag_mask: 1,
            is_favorite: false,
            content_length: text.len() as i32,
            created_at: "2026-07-01T12:00:00.000".into(),
            updated_at: "2026-07-01T12:30:00.000".into(),
            image_path: None,
        }
    }

    #[test]
    fn test_delete_entry_removes_and_adjusts_selection() {
        let repo = setup_repo();
        let id = repo.upsert_entry("del_hash", "exe", "title", 1, 10).unwrap();
        let mut entries = vec![make_entry(id, "hello")];
        let mut sel = 0usize;

        delete_entry(&repo, &mut entries, 0, &mut sel).unwrap();
        assert!(entries.is_empty());
        assert_eq!(sel, 0);
    }

    #[test]
    fn test_delete_entry_adjusts_selection_down() {
        let repo = setup_repo();
        let id1 = repo.upsert_entry("a", "exe", "t", 1, 1).unwrap();
        let id2 = repo.upsert_entry("b", "exe", "t", 1, 1).unwrap();
        let mut entries = vec![make_entry(id1, "a"), make_entry(id2, "b")];
        let mut sel = 1usize;

        delete_entry(&repo, &mut entries, 1, &mut sel).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(sel, 0); // was 1, removed last, clamped
    }

    #[test]
    fn test_toggle_favorite_flips_flag() {
        let repo = setup_repo();
        let id = repo.upsert_entry("fav", "exe", "t", 1, 5).unwrap();

        let entry_row = repo
            .get_history(0, "", 0, None, "updated_at", "desc", 10)
            .unwrap()
            .into_iter()
            .find(|e| e.id == id)
            .unwrap();

        let mut entry = entry_row;
        assert!(!entry.is_favorite);

        toggle_favorite(&repo, &mut entry).unwrap();
        assert!(entry.is_favorite);

        toggle_favorite(&repo, &mut entry).unwrap();
        assert!(!entry.is_favorite);
    }
}
