//! Operations (ops) module integration tests.
//!
//! Tests delete_entry, toggle_favorite, open_in_editor with real Repository.

mod common;

use jpastev2::ops;
use jpastev2::storage::repository::{Entry, Repository};

fn make_entry(repo: &Repository, text: &str, tag: i32) -> Entry {
    let hash = format!("op_{}", text);
    let eid = repo
        .upsert_entry(&hash, "test.exe", "Test", tag, text.len() as i32)
        .unwrap();
    if tag & 1 != 0 {
        repo.upsert_format(eid, 13, Some(text), None, &hash).unwrap();
    }
    Entry {
        id: eid,
        content_hash: hash,
        content: text.into(),
        source_exe: "test.exe".into(),
        source_title: "Test".into(),
        tag_mask: tag,
        is_favorite: false,
        content_length: text.len() as i32,
        created_at: "2026-07-01T12:00:00.000".into(),
        updated_at: "2026-07-01T12:30:00.000".into(),
        image_path: None,
    }
}

#[test]
fn test_delete_entry_removes_and_adjusts_selection() {
    let repo = common::temp_repo();
    let mut entries = vec![make_entry(&repo, "hello", 1)];
    let mut sel = 0usize;

    ops::delete_entry(&repo, &mut entries, 0, &mut sel).unwrap();
    assert!(entries.is_empty());
    assert_eq!(sel, 0);
}

#[test]
fn test_delete_entry_last_adjusts_selection_down() {
    let repo = common::temp_repo();
    let mut entries = vec![
        make_entry(&repo, "first", 1),
        make_entry(&repo, "second", 1),
    ];
    let mut sel = 1usize;

    ops::delete_entry(&repo, &mut entries, 1, &mut sel).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(sel, 0, "selection should clamp to last valid index");
}

#[test]
fn test_toggle_favorite_flips_flag() {
    let repo = common::temp_repo();
    let mut entry = make_entry(&repo, "fav_test", 1);

    assert!(!entry.is_favorite);
    ops::toggle_favorite(&repo, &mut entry).unwrap();
    assert!(entry.is_favorite);
    ops::toggle_favorite(&repo, &mut entry).unwrap();
    assert!(!entry.is_favorite);
}

#[test]
fn test_delete_out_of_range_is_noop() {
    let repo = common::temp_repo();
    let mut entries: Vec<Entry> = vec![];
    let mut sel = 0usize;

    ops::delete_entry(&repo, &mut entries, 0, &mut sel).unwrap();
    // no panic
}
