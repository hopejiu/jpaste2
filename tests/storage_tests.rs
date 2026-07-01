//! Storage layer integration tests.
//!
//! Tests the full lifecycle: upsert → query → paginate → delete → cleanup.

mod common;

use jpastev2::storage::repository::format;

#[test]
fn test_entry_lifecycle() {
    let repo = common::temp_repo();

    // Create
    let eid = repo.upsert_entry("h1", "notepad.exe", "Untitled", 1, 5).unwrap();
    assert!(eid > 0);

    // Add format
    repo.upsert_format(eid, format::CF_UNICODETEXT, Some("hello"), None, "fh1")
        .unwrap();
    let content = repo.get_format_content(eid, format::CF_UNICODETEXT).unwrap();
    assert_eq!(content, Some("hello".into()));

    // Dedup — same hash returns same id
    let eid2 = repo.upsert_entry("h1", "other.exe", "Other", 4, 0).unwrap();
    assert_eq!(eid, eid2);

    // Exists
    assert!(repo.exists_by_hash("h1").unwrap());
    assert!(!repo.exists_by_hash("nonexistent").unwrap());

    // Toggle favorite
    assert!(repo.toggle_favorite(eid).unwrap());
    assert!(!repo.toggle_favorite(eid).unwrap());

    // Delete
    repo.delete_entry(eid).unwrap();
    assert!(!repo.exists_by_hash("h1").unwrap());
}

#[test]
fn test_cursor_pagination_desc() {
    let repo = common::temp_repo();
    // Insert 5 entries
    for i in 0..5 {
        let hash = format!("h{i}");
        repo.upsert_entry(&hash, "exe", "title", 1, i).unwrap();
    }

    // First page (3 items)
    let page1 = repo.get_history(0, "", 0, None, "updated_at", "desc", 3).unwrap();
    assert_eq!(page1.len(), 3);

    // Second page using cursor from last item
    let last = page1.last().unwrap();
    let page2 = repo
        .get_history(0, &last.updated_at, last.id, None, "updated_at", "desc", 3)
        .unwrap();
    assert!(!page2.is_empty());

    // Total distinct items across pages
    let mut ids: Vec<i64> = page1.iter().chain(page2.iter()).map(|e| e.id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 5, "should see all 5 across two pages");
}

#[test]
fn test_cursor_pagination_asc() {
    let repo = common::temp_repo();
    for i in 0..5 {
        let hash = format!("h{i}");
        repo.upsert_entry(&hash, "exe", "title", 1, i).unwrap();
    }

    // Sort by content_length ascending
    let page1 = repo.get_history(0, "", 0, None, "content_length", "asc", 3).unwrap();
    assert_eq!(page1.len(), 3);
    assert!(page1[0].content_length <= page1[1].content_length);
}

#[test]
fn test_tag_filter() {
    let repo = common::temp_repo();
    repo.upsert_entry("t1", "exe", "t", 1, 0).unwrap(); // text
    repo.upsert_entry("t2", "exe", "t", 4, 0).unwrap(); // image
    repo.upsert_entry("t3", "exe", "t", 8, 0).unwrap(); // url

    let text_only = repo.get_history(1, "", 0, None, "updated_at", "desc", 10).unwrap();
    assert_eq!(text_only.len(), 1);

    let all = repo.get_history(0, "", 0, None, "updated_at", "desc", 10).unwrap();
    assert_eq!(all.len(), 3);
}

#[test]
fn test_search_filter() {
    let repo = common::temp_repo();
    let eid = repo.upsert_entry("s1", "exe", "t", 1, 5).unwrap();
    repo.upsert_format(eid, format::CF_UNICODETEXT, Some("hello world"), None, "f1")
        .unwrap();
    let eid2 = repo.upsert_entry("s2", "exe", "t", 1, 3).unwrap();
    repo.upsert_format(eid2, format::CF_UNICODETEXT, Some("bye"), None, "f2")
        .unwrap();

    let results = repo.get_history(0, "", 0, Some("hello"), "updated_at", "desc", 10).unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn test_cleanup_skips_favorites() {
    let repo = common::temp_repo();

    // Set an old updated_at by manipulating the DB directly,
    // since cleanup only removes entries older than now - retain_days.
    let keep_id = repo.upsert_entry("keep", "e", "t", 1, 0).unwrap();
    let fav_id = repo.upsert_entry("fav", "e", "t", 1, 0).unwrap();

    // Manually age the "keep" entry so it qualifies for cleanup
    {
        use rusqlite::params;
        let conn = &repo.db;
        let guard = conn.conn.lock().unwrap();
        guard
            .execute(
                "UPDATE clipboard_entry SET updated_at = '2020-01-01T00:00:00.000' WHERE id = ?1",
                params![keep_id],
            )
            .unwrap();
    }

    repo.toggle_favorite(fav_id).unwrap();

    // Cleanup should only remove non-favorites with old updated_at
    let _ = repo.cleanup(0).unwrap();
    assert!(!repo.exists_by_hash("keep").unwrap());
    assert!(repo.exists_by_hash("fav").unwrap());
}

#[test]
fn test_get_stats() {
    let repo = common::temp_repo();
    repo.upsert_entry("a", "e", "t", 1, 100).unwrap();
    repo.upsert_entry("b", "e", "t", 1, 200).unwrap();
    let (count, bytes) = repo.get_stats().unwrap();
    assert_eq!(count, 2);
    assert_eq!(bytes, 300);
}

#[test]
fn test_delete_all() {
    let repo = common::temp_repo();
    repo.upsert_entry("a", "e", "t", 1, 10).unwrap();
    repo.upsert_entry("b", "e", "t", 1, 20).unwrap();
    let _paths = repo.delete_all(false).unwrap();
    assert_eq!(repo.get_stats().unwrap().0, 0);
}

#[test]
fn test_delete_all_keep_favorites() {
    let repo = common::temp_repo();
    let id = repo.upsert_entry("fav", "e", "t", 1, 0).unwrap();
    repo.toggle_favorite(id).unwrap();
    repo.upsert_entry("other", "e", "t", 1, 0).unwrap();

    let _ = repo.delete_all(true).unwrap();
    let (count, _) = repo.get_stats().unwrap();
    assert_eq!(count, 1);
}
