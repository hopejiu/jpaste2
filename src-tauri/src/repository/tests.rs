//! Tests for the `Repository` data-access layer.

#[cfg(test)]
mod repository_tests {
    use crate::model;
    use crate::repository::Repository;

    #[test]
    fn test_get_image_list_empty() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let repo = Repository::new(&db_path).unwrap();
        let result = repo.get_image_list(0, "").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_image_list_with_entries() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let repo = Repository::new(&db_path).unwrap();

        // Insert an image entry
        repo.insert_entry("hash1", "test image", crate::model::TAG_IMAGE, 0, "")
            .unwrap();
        let id = repo.db.last_insert_rowid();
        repo.set_image_path(id, "images/2026-01-01/test.png")
            .unwrap();

        // Insert a text entry
        repo.insert_entry("hash2", "text content", crate::model::TAG_TEXT, 0, "")
            .unwrap();

        let result = repo.get_image_list(0, "").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], id);
    }
}

#[cfg(test)]
mod tests {
    use crate::model;
    use crate::repository::Repository;
    use rusqlite::params;
    use tempfile::TempDir;

    fn setup_repo() -> (Repository, TempDir) {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let repo = Repository::new(&db_path).unwrap();
        (repo, dir)
    }

    #[test]
    fn test_migration_creates_tables() {
        let (repo, _dir) = setup_repo();
        // Verify table exists by querying
        let count = repo
            .db
            .query_row("SELECT COUNT(*) FROM entries", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_insert_and_get_entry() {
        let (repo, _dir) = setup_repo();
        let hash = "abcdef123456";
        let id = repo
            .insert_entry(hash, "hello world", model::TAG_TEXT, 11, "")
            .unwrap();
        assert!(id > 0);

        let entry = repo.get_entry(id).unwrap();
        assert_eq!(entry.content_hash, hash);
        assert_eq!(entry.content, "hello world");
        assert_eq!(entry.tag_mask, model::TAG_TEXT);
    }

    #[test]
    fn test_upsert_dedup_updates_existing() {
        let (repo, _dir) = setup_repo();
        let hash = "dup_hash_1";
        let id1 = repo
            .insert_entry(hash, "first content", model::TAG_TEXT, 13, "")
            .unwrap();

        // Upsert with same hash should update, not insert
        let (was_dedup, cc) = repo
            .upsert_dedup(
                hash,
                model::TAG_TEXT | model::TAG_URL,
                "updated content",
                15,
            )
            .unwrap();
        assert!(was_dedup, "should detect duplicate");
        assert_eq!(cc, 1, "dedup should increment copy_count to 1");

        // Verify only one entry exists
        let count: i64 = repo
            .db
            .query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        // Verify content was updated
        let entry = repo.get_entry(id1).unwrap();
        assert_eq!(entry.content, "updated content");
        assert_eq!(entry.tag_mask, model::TAG_TEXT | model::TAG_URL);
    }

    #[test]
    fn test_upsert_dedup_new_hash_returns_false() {
        let (repo, _dir) = setup_repo();
        let (was_dedup, cc) = repo
            .upsert_dedup("nonexistent", model::TAG_TEXT, "content", 7)
            .unwrap();
        assert!(!was_dedup, "new hash should not be dedup");
        assert_eq!(cc, 0, "no dedup → copy_count=0");
    }

    #[test]
    fn test_delete_entry() {
        let (repo, _dir) = setup_repo();
        let id = repo
            .insert_entry("hash_del", "delete me", model::TAG_TEXT, 8, "")
            .unwrap();

        let (img_path, thumb_path) = repo.delete_entry(id).unwrap();
        assert!(img_path.is_none());
        assert!(thumb_path.is_none());

        let count: i64 = repo
            .db
            .query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_delete_entry_with_image() {
        let (repo, _dir) = setup_repo();
        let id = repo
            .insert_entry("hash_img", "image entry", model::TAG_IMAGE, 0, "")
            .unwrap();
        repo.set_image_path(id, "images/2026-01-01/test.png")
            .unwrap();

        let (img_path, _thumb_path) = repo.delete_entry(id).unwrap();
        assert_eq!(img_path, Some("images/2026-01-01/test.png".to_string()));
    }

    #[test]
    fn test_toggle_favorite() {
        let (repo, _dir) = setup_repo();
        let id = repo.insert_entry("hash_fav", "fav test", 0, 8, "").unwrap();

        repo.toggle_favorite(id, true).unwrap();
        let entry = repo.get_entry(id).unwrap();
        assert!(entry.is_favorite);

        repo.toggle_favorite(id, false).unwrap();
        let entry = repo.get_entry(id).unwrap();
        assert!(!entry.is_favorite);
    }

    #[test]
    fn test_query_entries_pagination() {
        let (repo, _dir) = setup_repo();
        // Insert 5 entries with stable hash-like content for predictable order
        for i in 0..5 {
            let content = format!("entry_{}", i);
            let hash = format!("hash_{:020}", i);
            repo.insert_entry(&hash, &content, model::TAG_TEXT, content.len() as i64, "")
                .unwrap();
        }

        // Query first page (limit 3)
        let page1 = repo
            .query_entries("", 0, 0, 0, 3, "updated_at", "DESC")
            .unwrap();
        assert_eq!(page1.len(), 3, "first page should have 3 entries");

        // Query second page using cursor from last entry of page1
        let last = page1.last().unwrap();
        let page2 = repo
            .query_entries("", 0, last.updated_at, last.id, 3, "updated_at", "DESC")
            .unwrap();
        assert_eq!(page2.len(), 2, "second page should have 2 entries");
    }

    #[test]
    fn test_query_entries_tag_filter() {
        let (repo, _dir) = setup_repo();
        repo.insert_entry("hash_t1", "text only", model::TAG_TEXT, 9, "")
            .unwrap();
        repo.insert_entry(
            "hash_u1",
            "https://example.com",
            model::TAG_URL | model::TAG_TEXT,
            19,
            "",
        )
        .unwrap();

        let text_entries = repo
            .query_entries("", model::TAG_TEXT, 0, 0, 10, "updated_at", "DESC")
            .unwrap();
        assert_eq!(text_entries.len(), 2, "both match TAG_TEXT");

        let url_entries = repo
            .query_entries("", model::TAG_URL, 0, 0, 10, "updated_at", "DESC")
            .unwrap();
        assert_eq!(url_entries.len(), 1, "only one has URL tag");
    }

    #[test]
    fn test_query_entries_search() {
        let (repo, _dir) = setup_repo();
        repo.insert_entry("h1", "hello world", model::TAG_TEXT, 11, "")
            .unwrap();
        repo.insert_entry("h2", "foo bar", model::TAG_TEXT, 7, "")
            .unwrap();
        repo.insert_entry("h3", "nope nope", model::TAG_TEXT, 9, "")
            .unwrap();

        let results = repo
            .query_entries("hello", 0, 0, 0, 10, "updated_at", "DESC")
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "hello world");
    }

    #[test]
    fn test_cleanup() {
        let (repo, _dir) = setup_repo();
        repo.insert_entry("fresh", "recent entry", 0, 12, "").unwrap();

        // Manually set updated_at to be old
        repo.db
            .execute(
                "UPDATE entries SET updated_at = 1000 WHERE content_hash = ?2",
                params!["fresh"],
            )
            .unwrap();

        // Insert a favorite entry with old timestamp
        repo.insert_entry("old_fav", "old favorite", 0, 11, "").unwrap();
        repo.db
            .execute(
                "UPDATE entries SET updated_at = 1000, is_favorite = 1 WHERE content_hash = ?2",
                params!["old_fav"],
            )
            .unwrap();

        let (deleted, _paths) = repo.cleanup(1).unwrap(); // 1 day retention
        assert_eq!(deleted, 1, "should delete the non-favorite old entry");
    }

    #[test]
    fn test_clear_all() {
        let (repo, _dir) = setup_repo();
        repo.insert_entry("h1", "a", 0, 1, "").unwrap();
        repo.toggle_favorite(1, true).unwrap();
        repo.insert_entry("h2", "b", 0, 1, "").unwrap();

        let paths = repo.clear_all(false).unwrap();
        assert!(paths.is_empty());

        let count: i64 = repo
            .db
            .query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_clear_all_keep_favorites() {
        let (repo, _dir) = setup_repo();
        repo.insert_entry("h1", "fav", 0, 3, "").unwrap();
        repo.toggle_favorite(1, true).unwrap();
        repo.insert_entry("h2", "non-fav", 0, 7, "").unwrap();

        repo.clear_all(true).unwrap();

        let count: i64 = repo
            .db
            .query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1, "favorite should survive");
    }

    #[test]
    fn test_get_stats() {
        let (repo, _dir) = setup_repo();
        repo.insert_entry("h1", "hello", 0, 5, "").unwrap();
        repo.insert_entry("h2", "world!!!", 0, 8, "").unwrap();

        let stats = repo.get_stats().unwrap();
        assert_eq!(stats.count, 2);
        assert_eq!(stats.total_bytes, 13);
        assert_eq!(stats.image_bytes, 0);
    }

    #[test]
    fn test_get_image_storage_bytes() {
        let (repo, dir) = setup_repo();
        // Create an image file
        let img1 = image::RgbImage::new(10, 10);
        let img1_path = dir.path().join("img1.png");
        img1.save(&img1_path).unwrap();
        let img1_size = std::fs::metadata(&img1_path).unwrap().len() as i64;

        let id1 = repo.insert_entry("h1", "", model::TAG_IMAGE, 0, "").unwrap();
        repo.set_image_path(id1, "img1.png").unwrap();

        let bytes = repo.get_image_storage_bytes(dir.path()).unwrap();
        assert!(bytes >= img1_size, "should include image file size");
    }

    #[test]
    fn test_set_image_path() {
        let (repo, _dir) = setup_repo();
        let id = repo
            .insert_entry("img_test", "", model::TAG_IMAGE, 0, "")
            .unwrap();
        repo.set_image_path(id, "images/2026/abc.png").unwrap();

        let entry = repo.get_entry(id).unwrap();
        assert_eq!(entry.image_path, "images/2026/abc.png");
    }

    #[test]
    fn test_set_thumb_path() {
        let (repo, _dir) = setup_repo();
        let id = repo
            .insert_entry("thumb_test", "", model::TAG_IMAGE, 0, "")
            .unwrap();
        repo.set_thumb_path(id, "thumbs/abc.webp").unwrap();

        let entry = repo.get_entry(id).unwrap();
        assert_eq!(entry.thumb_path, "thumbs/abc.webp");
    }

    #[test]
    fn test_update_timestamp() {
        let (repo, _dir) = setup_repo();
        let id = repo
            .insert_entry("ts_update", "content", 0, 7, "")
            .unwrap();
        let old = repo.get_entry(id).unwrap().updated_at;

        std::thread::sleep(std::time::Duration::from_millis(10));
        repo.update_timestamp(id).unwrap();

        let updated = repo.get_entry(id).unwrap().updated_at;
        assert!(updated > old, "timestamp should increase");
    }

    #[test]
    fn test_increment_copy_count() {
        let (repo, _dir) = setup_repo();
        let id = repo
            .insert_entry("cc_test", "content", 0, 7, "")
            .unwrap();

        let cc = repo.increment_copy_count(id).unwrap();
        assert_eq!(cc, 1, "first increment should be 1");

        let cc = repo.increment_copy_count(id).unwrap();
        assert_eq!(cc, 2, "second increment should be 2");
    }

    #[test]
    fn test_set_qr_text() {
        let (repo, _dir) = setup_repo();
        let id = repo
            .insert_entry("qr_test", "content", 0, 7, "")
            .unwrap();

        repo.set_qr_text(id, "decoded_qr_value").unwrap();
        let entry = repo.get_entry(id).unwrap();
        assert_eq!(entry.qr_text, "decoded_qr_value");
    }

    #[test]
    fn test_find_id_by_hash() {
        let (repo, _dir) = setup_repo();
        let hash = "find_me_hash_123";
        let id = repo
            .insert_entry(hash, "content", 0, 7, "")
            .unwrap();

        let found = repo.find_id_by_hash(hash).unwrap();
        assert_eq!(found, id, "should find existing hash");

        let not_found = repo.find_id_by_hash("nonexistent").unwrap();
        assert_eq!(not_found, 0, "should return 0 for missing hash");
    }

    #[test]
    fn test_set_no_auto_fav_and_has_flag() {
        let (repo, _dir) = setup_repo();
        let id = repo
            .insert_entry("naf_test", "content", 0, 7, "")
            .unwrap();

        // Initially false
        assert!(!repo.has_no_auto_fav_flag(id).unwrap());

        // Set to true
        repo.set_no_auto_fav(id, true).unwrap();
        assert!(repo.has_no_auto_fav_flag(id).unwrap());

        // Set back to false
        repo.set_no_auto_fav(id, false).unwrap();
        assert!(!repo.has_no_auto_fav_flag(id).unwrap());
    }
}
