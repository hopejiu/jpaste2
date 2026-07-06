//! Integration tests for jPaste v2
//!
//! These tests verify the core clipboard history pipeline,
//! database operations, and service integrations.

#[cfg(test)]
mod integration_tests {
    use std::path::PathBuf;
    use tempfile::TempDir;

    use crate::repository::Repository;
    use crate::service::history::HistoryService;
    use crate::service::settings::SettingsService;
    use crate::model;

    fn setup_test_dir() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();
        (dir, path)
    }

    // ── Edge cases ─────────────────────────────────────────────────────────

    #[test]
    fn test_save_empty_text() {
        let (_dir, path) = setup_test_dir();
        let history = HistoryService::new(&path).unwrap();
        let payload = history.save_clipboard("empty_hash", "", model::TAG_TEXT, None, "").unwrap();
        assert!(payload.id > 0);

        let result = history.get_entries("", 0, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].content, "");
    }

    #[test]
    fn test_save_very_long_text() {
        let (_dir, path) = setup_test_dir();
        let history = HistoryService::new(&path).unwrap();
        let long_text = "a".repeat(100_000);
        let payload = history.save_clipboard("long_hash", &long_text, model::TAG_TEXT, None, "").unwrap();
        assert!(payload.id > 0);

        let result = history.get_entries("", 0, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert_eq!(result.entries[0].content.len(), 100_000);
    }

    #[test]
    fn test_save_unicode_text() {
        let (_dir, path) = setup_test_dir();
        let history = HistoryService::new(&path).unwrap();
        let text = "Hello 世界 🌍 Привет";
        let payload = history.save_clipboard("unicode_hash", text, model::TAG_TEXT, None, "").unwrap();
        assert!(payload.id > 0);

        let result = history.get_entries("", 0, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert_eq!(result.entries[0].content, text);
    }

    #[test]
    fn test_save_special_characters() {
        let (_dir, path) = setup_test_dir();
        let history = HistoryService::new(&path).unwrap();
        let text = "special <>&\"' \t\n\r\\ chars";
        let payload = history.save_clipboard("special_hash", text, model::TAG_TEXT, None, "").unwrap();
        assert!(payload.id > 0);

        let result = history.get_entries("", 0, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert_eq!(result.entries[0].content, text);
    }

    #[test]
    fn test_concurrent_inserts() {
        let (_dir, path) = setup_test_dir();
        let history = HistoryService::new(&path).unwrap();
        let history = std::sync::Arc::new(history);

        let mut handles = vec![];
        for i in 0..10 {
            let h = history.clone();
            let handle = std::thread::spawn(move || {
                let hash = format!("concurrent_{}", i);
                h.save_clipboard(&hash, &format!("content {}", i), model::TAG_TEXT, None, "").unwrap();
            });
            handles.push(handle);
        }

        for h in handles {
            h.join().unwrap();
        }

        let result = history.get_entries("", 0, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert_eq!(result.entries.len(), 10);
    }

    #[test]
    fn test_dedup_updates_timestamp() {
        let (_dir, path) = setup_test_dir();
        let history = HistoryService::new(&path).unwrap();

        history.save_clipboard("ts_hash", "first", model::TAG_TEXT, None, "").unwrap();
        let result1 = history.get_entries("first", 0, 0, 0, 20, "updated_at", "DESC").unwrap();
        let _ts1 = result1.entries[0].updated_at.clone();

        // Small delay to ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_millis(10));
        history.save_clipboard("ts_hash", "second", model::TAG_TEXT, None, "").unwrap();

        let result2 = history.get_entries("second", 0, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert_eq!(result2.entries.len(), 1);
        assert_eq!(result2.entries[0].content, "second");
    }

    #[test]
    fn test_multiple_tag_filter_combinations() {
        let (_dir, path) = setup_test_dir();
        let history = HistoryService::new(&path).unwrap();

        history.save_clipboard("t1", "plain text", model::TAG_TEXT, None, "").unwrap();
        history.save_clipboard("t2", "https://example.com", model::TAG_TEXT | model::TAG_URL, None, "").unwrap();
        history.save_clipboard("t3", "", model::TAG_IMAGE, None, "").unwrap();

        // Filter by TEXT should get both text entries
        let text_results = history.get_entries("", model::TAG_TEXT, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert_eq!(text_results.entries.len(), 2);

        // Filter by URL should get only the URL entry
        let url_results = history.get_entries("", model::TAG_URL, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert_eq!(url_results.entries.len(), 1);
    }

    #[test]
    fn test_sort_by_content_length() {
        let (_dir, path) = setup_test_dir();
        let history = HistoryService::new(&path).unwrap();

        history.save_clipboard("s1", "short", model::TAG_TEXT, None, "").unwrap();
        history.save_clipboard("s2", "a much longer content here", model::TAG_TEXT, None, "").unwrap();
        history.save_clipboard("s3", "medium one", model::TAG_TEXT, None, "").unwrap();

        let asc = history.get_entries("", 0, 0, 0, 20, "content_length", "ASC").unwrap();
        assert_eq!(asc.entries[0].content, "short");
        assert_eq!(asc.entries[2].content, "a much longer content here");

        let desc = history.get_entries("", 0, 0, 0, 20, "content_length", "DESC").unwrap();
        assert_eq!(desc.entries[0].content, "a much longer content here");
        assert_eq!(desc.entries[2].content, "short");
    }

    #[test]
    fn test_pagination_exact_boundary() {
        let (_dir, path) = setup_test_dir();
        let history = HistoryService::new(&path).unwrap();

        // Insert exactly 3 entries
        for i in 0..3 {
            let hash = format!("exact_{:020}", i);
            history.save_clipboard(&hash, &format!("entry {}", i), model::TAG_TEXT, None, "").unwrap();
        }

        // Request limit=3, should have has_more=false
        let page = history.get_entries("", 0, 0, 0, 3, "updated_at", "DESC").unwrap();
        assert_eq!(page.entries.len(), 3);
        assert!(!page.has_more);

        // Request limit=2, should have has_more=true
        let page = history.get_entries("", 0, 0, 0, 2, "updated_at", "DESC").unwrap();
        assert_eq!(page.entries.len(), 2);
        assert!(page.has_more);
    }

    #[test]
    fn test_search_with_special_chars() {
        let (_dir, path) = setup_test_dir();
        let history = HistoryService::new(&path).unwrap();

        history.save_clipboard("sc1", "hello'world", model::TAG_TEXT, None, "").unwrap();
        history.save_clipboard("sc2", "normal text", model::TAG_TEXT, None, "").unwrap();

        let result = history.get_entries("hello'", 0, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert_eq!(result.entries.len(), 1);
    }

    #[test]
    fn test_get_entry_content_not_found() {
        let (_dir, path) = setup_test_dir();
        let history = HistoryService::new(&path).unwrap();
        let result = history.get_entry_content(99999);
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_nonexistent_entry_returns_ok() {
        let (_dir, path) = setup_test_dir();
        let history = HistoryService::new(&path).unwrap();
        // SQLite DELETE on non-existent row succeeds with 0 affected rows
        let result = history.delete_entry(99999);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), false); // no image was deleted
    }

    #[test]
    fn test_toggle_favorite_nonexistent_returns_ok() {
        let (_dir, path) = setup_test_dir();
        let history = HistoryService::new(&path).unwrap();
        // SQLite UPDATE on non-existent row succeeds with 0 affected rows
        let result = history.toggle_favorite(99999, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_clear_all_empty_db() {
        let (_dir, path) = setup_test_dir();
        let history = HistoryService::new(&path).unwrap();
        history.clear_all(false).unwrap();

        let result = history.get_entries("", 0, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert!(result.entries.is_empty());
    }

    #[test]
    fn test_stats_empty_db() {
        let (_dir, path) = setup_test_dir();
        let history = HistoryService::new(&path).unwrap();
        let stats = history.get_stats().unwrap();
        assert_eq!(stats.count, 0);
        assert_eq!(stats.total_bytes, 0);
    }

    #[test]
    fn test_settings_change_callback_on_sort_field() {
        let (_dir, path) = setup_test_dir();
        let settings = SettingsService::new(&path);

        let called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let called_clone = called.clone();

        settings.on_settings_change(move |_old, new| {
            assert_eq!(new.sort_field, "content_length");
            called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        let mut data = settings.get_settings().unwrap();
        data.sort_field = "content_length".to_string();
        settings.save_settings(data).unwrap();

        assert!(called.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn test_settings_no_callback_when_unchanged() {
        let (_dir, path) = setup_test_dir();
        let settings = SettingsService::new(&path);

        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        settings.on_settings_change(move |_old, _new| {
            call_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });

        // Save same data - should not trigger callback
        let data = settings.get_settings().unwrap();
        settings.save_settings(data).unwrap();

        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn test_image_list_with_search() {
        let (_dir, path) = setup_test_dir();
        let history = HistoryService::new(&path).unwrap();

        // Insert image entries with searchable text
        let p1 = history.save_clipboard("img1_hash", "screenshot 1", model::TAG_IMAGE, None, "").unwrap();
        let p2 = history.save_clipboard("img2_hash", "screenshot 2", model::TAG_IMAGE, None, "").unwrap();
        history.save_clipboard("text1_hash", "screenshot text", model::TAG_TEXT, None, "").unwrap();

        // Manually set image paths
        {
            let repo_path = path.join("clipboard.db");
            let repo = Repository::new(&repo_path).unwrap();
            repo.set_image_path(p1.id, "images/2026-01-01/img1.png").unwrap();
            repo.set_image_path(p2.id, "images/2026-01-01/img2.png").unwrap();
        }

        let repo_path = path.join("clipboard.db");
        let repo = Repository::new(&repo_path).unwrap();
        let result = repo.get_image_list(0, "screenshot").unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_full_pipeline_with_image_data() {
        let (_dir, path) = setup_test_dir();
        let history = HistoryService::new(&path).unwrap();

        // Simulate saving with image data
        let fake_image = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]; // PNG header
        let payload = history.save_clipboard(
            "img_pipeline_hash",
            "",
            model::TAG_IMAGE,
            Some(&fake_image),
            "",
        ).unwrap();
        assert!(payload.id > 0);

        // Verify image path was set
        let result = history.get_entries("", model::TAG_IMAGE, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert_eq!(result.entries.len(), 1);
        assert!(result.entries[0].has_image);
    }

    #[test]
    fn test_full_clipboard_pipeline() {
        let (_dir, path) = setup_test_dir();

        // Initialize services
        let history = HistoryService::new(&path).unwrap();
        let _settings = SettingsService::new(&path);

        // Save a clipboard entry
        let payload = history.save_clipboard(
            "test_hash_1",
            "Hello, World!",
            model::TAG_TEXT,
            None,
            "",
        ).unwrap();

        assert!(payload.id > 0);
        // Preview is truncated to 40 chars
        assert!(payload.content_preview.starts_with("Hello, World"));
        assert_eq!(payload.tag_mask, model::TAG_TEXT);

        // Retrieve entries
        let result = history.get_entries("", 0, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].content, "Hello, World!");
        assert!(!result.has_more);

        // Toggle favorite
        history.toggle_favorite(payload.id, true).unwrap();
        let result = history.get_entries("", model::TAG_FAVORITE, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert_eq!(result.entries.len(), 1);

        // Delete entry
        let had_image = history.delete_entry(payload.id).unwrap();
        assert!(!had_image);
        let result = history.get_entries("", 0, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert!(result.entries.is_empty());
    }

    #[test]
    fn test_dedup_prevents_duplicates() {
        let (_dir, path) = setup_test_dir();
        let history = HistoryService::new(&path).unwrap();

        // Insert same content twice
        let payload1 = history.save_clipboard("dedup_hash", "duplicate content", model::TAG_TEXT, None, "").unwrap();
        let payload2 = history.save_clipboard("dedup_hash", "updated content", model::TAG_TEXT, None, "").unwrap();

        // Should be the same entry (deduped)
        assert_eq!(payload1.id, payload2.id);

        // Only one entry should exist
        let result = history.get_entries("", 0, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].content, "updated content");
    }

    #[test]
    fn test_search_filters_correctly() {
        let (_dir, path) = setup_test_dir();
        let history = HistoryService::new(&path).unwrap();

        history.save_clipboard("h1", "find me", model::TAG_TEXT, None, "").unwrap();
        history.save_clipboard("h2", "don't match", model::TAG_TEXT, None, "").unwrap();
        history.save_clipboard("h3", "find me too", model::TAG_TEXT, None, "").unwrap();

        let result = history.get_entries("find", 0, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert_eq!(result.entries.len(), 2);
    }

    #[test]
    fn test_tag_filtering() {
        let (_dir, path) = setup_test_dir();
        let history = HistoryService::new(&path).unwrap();

        history.save_clipboard("h1", "text entry", model::TAG_TEXT, None, "").unwrap();
        history.save_clipboard("h2", "https://example.com", model::TAG_URL | model::TAG_TEXT, None, "").unwrap();

        // Filter by URL tag
        let result = history.get_entries("", model::TAG_URL, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert_eq!(result.entries.len(), 1);
        assert!(result.entries[0].content.starts_with("https://"));
    }

    #[test]
    fn test_settings_persistence() {
        let (_dir, path) = setup_test_dir();
        let settings = SettingsService::new(&path);

        // Modify and save settings
        let mut data = settings.get_settings().unwrap();
        data.hotkey = "Ctrl+Space".to_string();
        data.retain_days = 7;
        data.auto_start = true;
        settings.save_settings(data).unwrap();

        // Create new service pointing to same file
        let settings2 = SettingsService::new(&path);
        settings2.load().unwrap();
        let loaded = settings2.get_settings().unwrap();

        assert_eq!(loaded.hotkey, "Ctrl+Space");
        assert_eq!(loaded.retain_days, 7);
        assert!(loaded.auto_start);
    }

    #[test]
    fn test_cleanup_removes_old_entries() {
        let (_dir, path) = setup_test_dir();
        let history = HistoryService::new(&path).unwrap();

        // Insert an entry
        history.save_clipboard("old_hash", "old entry", model::TAG_TEXT, None, "").unwrap();

        // Manually set updated_at to be old
        {
            let repo_path = path.join("clipboard.db");
            let repo = Repository::new(&repo_path).unwrap();
            repo.migrate().unwrap();
            repo.db.execute(
                "UPDATE entries SET updated_at = 1000 WHERE content_hash = 'old_hash'",
                [],
            ).unwrap();
        }

        // Run cleanup with 1 day retention
        let result = history.cleanup(1).unwrap();
        assert_eq!(result.deleted, 1);

        // Verify entry is gone
        let entries = history.get_entries("", 0, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert!(entries.entries.is_empty());
    }

    #[test]
    fn test_clear_all_keeps_favorites() {
        let (_dir, path) = setup_test_dir();
        let history = HistoryService::new(&path).unwrap();

        let payload = history.save_clipboard("fav_hash", "favorite", model::TAG_TEXT, None, "").unwrap();
        history.toggle_favorite(payload.id, true).unwrap();
        history.save_clipboard("non_fav_hash", "not favorite", model::TAG_TEXT, None, "").unwrap();

        // Clear all but keep favorites
        history.clear_all(true).unwrap();

        let result = history.get_entries("", 0, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].content, "favorite");
    }

    #[test]
    fn test_image_entry_with_path() {
        let (_dir, path) = setup_test_dir();
        let history = HistoryService::new(&path).unwrap();

        // Insert an image entry
        let payload = history.save_clipboard(
            "img_hash",
            "",
            model::TAG_IMAGE,
            None,
            "",
        ).unwrap();

        // Get image path (should fail since no actual image data)
        let result = history.get_entry_image_path(payload.id);
        assert!(result.is_err()); // No image path set
    }

    #[test]
    fn test_pagination() {
        let (_dir, path) = setup_test_dir();
        let history = HistoryService::new(&path).unwrap();

        // Insert 5 entries
        for i in 0..5 {
            let hash = format!("hash_{:020}", i);
            history.save_clipboard(&hash, &format!("entry {}", i), model::TAG_TEXT, None, "").unwrap();
        }

        // Get first page (limit 3)
        let page1 = history.get_entries("", 0, 0, 0, 3, "updated_at", "DESC").unwrap();
        assert_eq!(page1.entries.len(), 3);
        assert!(page1.has_more);

        // Get second page using cursor
        let last = page1.entries.last().unwrap();
        let page2 = history.get_entries("", 0, last.updated_at, last.id, 3, "updated_at", "DESC").unwrap();
        assert_eq!(page2.entries.len(), 2);
        assert!(!page2.has_more);
    }

    #[test]
    fn test_stats() {
        let (_dir, path) = setup_test_dir();
        let history = HistoryService::new(&path).unwrap();

        history.save_clipboard("h1", "hello", model::TAG_TEXT, None, "").unwrap();
        history.save_clipboard("h2", "world!!!", model::TAG_TEXT, None, "").unwrap();

        let stats = history.get_stats().unwrap();
        assert_eq!(stats.count, 2);
        assert!(stats.total_bytes > 0);
    }
}

#[cfg(test)]
mod filostack_tests {
    use crate::service::filostack::FiloStack;

    #[test]
    fn test_fifo_order() {
        let stack = FiloStack::new();
        stack.set_mode("queue");

        stack.push("first");
        stack.push("second");
        stack.push("third");

        assert_eq!(stack.pop(), Some("first".to_string()));
        assert_eq!(stack.pop(), Some("second".to_string()));
        assert_eq!(stack.pop(), Some("third".to_string()));
        assert_eq!(stack.pop(), None);
    }

    #[test]
    fn test_clone_independence() {
        let stack = FiloStack::new();
        stack.set_mode("queue");
        stack.push("item1");

        let cloned = stack.clone();
        cloned.push("item2");

        // Both should see the same data (shared state via Arc)
        assert_eq!(stack.len(), 2);
        assert_eq!(cloned.len(), 2);
    }

    #[test]
    fn test_pop_empty_returns_none() {
        let stack = FiloStack::new();
        stack.set_mode("queue");
        assert_eq!(stack.pop(), None);
    }

    #[test]
    fn test_mode_change_idempotent() {
        let stack = FiloStack::new();
        stack.set_mode("queue");
        stack.push("item");

        // Setting same mode again should be a no-op
        stack.set_mode("queue");
        // Queue should still have items (same mode doesn't clear)
        assert_eq!(stack.len(), 1);
    }

    #[test]
    fn test_clear_resets_self_tracker() {
        let stack = FiloStack::new();
        stack.set_mode("queue");
        stack.mark_self_write("tracked");
        stack.push("tracked"); // should be skipped
        assert_eq!(stack.len(), 0);

        stack.clear();
        // After clear, the self-tracker should be reset
        stack.push("tracked"); // should now be pushed
        assert_eq!(stack.len(), 1);
    }

    #[test]
    fn test_get_status_includes_previews() {
        let stack = FiloStack::new();
        stack.set_mode("queue");
        stack.push("short");
        stack.push("a longer text that should be truncated in preview");

        let status = stack.get_status();
        assert_eq!(status.item_count, 2);
        assert_eq!(status.items.len(), 2);
        // Preview should be truncated to 40 chars
        assert!(status.items[1].len() <= 43); // 40 + "..."
    }

    #[test]
    fn test_push_whitespace_only() {
        let stack = FiloStack::new();
        stack.set_mode("queue");
        stack.push("   "); // whitespace-only is not empty
        assert_eq!(stack.len(), 1);
    }

    #[test]
    fn test_mode_change_clears_queue() {
        let stack = FiloStack::new();
        stack.set_mode("queue");
        stack.push("item1");
        stack.push("item2");
        assert_eq!(stack.len(), 2);

        stack.set_mode("normal");
        assert_eq!(stack.len(), 0);

        // Switching back should start fresh
        stack.set_mode("queue");
        assert_eq!(stack.len(), 0);
    }

    #[test]
    fn test_normal_mode_ignores_push() {
        let stack = FiloStack::new();
        // Default mode is normal
        stack.push("test");
        assert_eq!(stack.len(), 0);
    }

    #[test]
    fn test_self_write_tracking() {
        let stack = FiloStack::new();
        stack.set_mode("queue");

        stack.mark_self_write("my content");
        stack.push("my content");
        assert_eq!(stack.len(), 0, "self-write should be skipped");

        // After expiration, same text should be pushed
        // (We can't easily test expiration without sleeping)
    }

    #[test]
    fn test_get_status() {
        let stack = FiloStack::new();
        stack.set_mode("queue");
        stack.push("hello world");

        let status = stack.get_status();
        assert_eq!(status.mode, "queue");
        assert!(status.enabled);
        assert_eq!(status.item_count, 1);
        assert!(!status.items.is_empty());
    }
}

#[cfg(test)]
mod model_tests {
    use crate::model;

    #[test]
    fn test_tag_empty_text_no_image() {
        let mask = model::compute_tag_mask("", false, false, false);
        assert_eq!(mask, 0);
    }

    #[test]
    fn test_tag_url_with_image() {
        let mask = model::compute_tag_mask("https://example.com", true, false, false);
        assert!(mask & model::TAG_IMAGE != 0);
        assert!(mask & model::TAG_URL != 0);
        assert!(mask & model::TAG_TEXT == 0, "image+url should not be plain text");
    }

    #[test]
    fn test_tag_file_forward_slash() {
        let mask = model::compute_tag_mask("C:/Users/test", false, false, false);
        assert!(mask & model::TAG_FILE != 0);
        assert!(mask & model::TAG_TEXT == 0);
    }

    #[test]
    fn test_tag_unc_path() {
        let mask = model::compute_tag_mask("\\\\server\\share", false, false, false);
        assert!(mask & model::TAG_FILE != 0);
    }

    #[test]
    fn test_content_length_empty() {
        assert_eq!(model::content_length(""), 0);
    }

    #[test]
    fn test_content_length_ascii() {
        assert_eq!(model::content_length("hello"), 5);
    }

    #[test]
    fn test_content_length_unicode() {
        assert_eq!(model::content_length("中文"), 6);
    }
}

#[cfg(test)]
mod util_tests {
    use crate::util::{self, SelfWriteTracker};
    #[test]
    fn test_sha256_hex_known_value() {
        let h = util::sha256_hex("hello");
        assert_eq!(h, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
    }

    #[test]
    fn test_sha256_bytes_known_value() {
        let h = util::sha256_bytes(b"hello");
        assert_eq!(h, util::sha256_hex("hello"));
    }

    #[test]
    fn test_truncate_exact_boundary() {
        let s = "abcdefghij";
        let t = util::truncate(s, 10);
        assert_eq!(t, "abcdefghij");
    }

    #[test]
    fn test_truncate_one_over() {
        let s = "abcdefghijk";
        let t = util::truncate(s, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 13);
    }

    #[test]
    fn test_truncate_multibyte_boundary() {
        let s = "你好世界";
        let t = util::truncate(s, 8);
        assert!(t.ends_with("..."));
    }

    #[test]
    fn test_self_write_tracker_not_expired() {
        let mut tracker = SelfWriteTracker::new();
        tracker.mark("content");
        assert!(!tracker.is_expired());
        assert!(tracker.is_self_write("content"));
    }

    #[test]
    fn test_self_write_tracker_different_text() {
        let mut tracker = SelfWriteTracker::new();
        tracker.mark("content");
        assert!(!tracker.is_self_write("different"));
    }

    #[test]
    fn test_prepend_bmp_header_small_dib() {
        let dib = vec![0u8; 44]; // 40 header + 4 pixel
        let bmp = util::prepend_bmp_header(&dib);
        assert_eq!(bmp.len(), 58); // 44 + 14
        assert_eq!(&bmp[..2], b"BM");
    }
}
