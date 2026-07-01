use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Mutex;

use crate::model;
use crate::repository::Repository;

/// HistoryService handles clipboard history CRUD operations
pub struct HistoryService {
    pub(crate) repo: Mutex<Repository>,
    app_data: String,
    /// Cached total image file size (bytes) — updated incrementally on insert/delete.
    image_bytes_cache: AtomicI64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryResponse {
    pub id: i64,
    pub content_hash: String,
    pub content: String,
    pub content_preview: String,
    pub image_path: String,
    pub thumb_path: String,
    pub has_image: bool,
    pub tag_mask: i32,
    pub is_favorite: bool,
    pub content_length: i64,
    pub copy_count: i64,
    pub qr_text: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub entries: Vec<EntryResponse>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupResult {
    pub deleted: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsResponse {
    pub count: i64,
    pub total_bytes: i64,
    pub image_bytes: i64,
}

impl HistoryService {
    pub fn new(app_data: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        log::info!("history: initializing at {:?}", app_data);
        let db_path = app_data.join("clipboard.db");
        let repo = Repository::new(&db_path)?;
        // Initialize image_bytes_cache from existing data
        let image_bytes_cache = AtomicI64::new(repo.get_image_storage_bytes(app_data).unwrap_or(0));
        Ok(Self {
            repo: Mutex::new(repo),
            app_data: app_data.to_string_lossy().to_string(),
            image_bytes_cache,
        })
    }

    fn entry_to_response(e: crate::repository::EntryRow) -> EntryResponse {
        let preview = crate::util::truncate(&e.content, 120);
        EntryResponse {
            id: e.id,
            content_hash: e.content_hash,
            content: e.content,
            content_preview: preview,
            image_path: e.image_path.clone(),
            thumb_path: e.thumb_path.clone(),
            has_image: !e.image_path.is_empty(),
            tag_mask: e.tag_mask,
            is_favorite: e.is_favorite,
            content_length: e.content_length,
            copy_count: e.copy_count,
            qr_text: e.qr_text,
            created_at: e.created_at,
            updated_at: e.updated_at,
        }
    }

    /// Get entries with pagination, filtering, and search
    pub fn get_entries(
        &self,
        search: &str,
        tag_mask: i32,
        cursor_updated: i64,
        cursor_id: i64,
        limit: i32,
        sort_field: &str,
        sort_order: &str,
    ) -> Result<QueryResult, String> {
        let repo = self.repo.lock().map_err(|e| e.to_string())?;

        // Fetch limit+1 to determine if there are more pages
        let rows = repo
            .query_entries(search, tag_mask, cursor_updated, cursor_id, limit + 1, sort_field, sort_order)
            .map_err(|e| e.to_string())?;

        let has_more = rows.len() as i32 > limit;
        let entries: Vec<EntryResponse> = rows
            .into_iter()
            .take(limit as usize)
            .map(Self::entry_to_response)
            .collect();

        Ok(QueryResult { entries, has_more })
    }

    /// Get entries matching a regex pattern. Paginates internally in batches.
    /// Uses lightweight query (id+content only) to avoid loading full rows.
    pub fn get_entries_regex(
        &self,
        pattern: &str,
        tag_mask: i32,
        sort_field: &str,
        sort_order: &str,
    ) -> Result<Vec<EntryResponse>, String> {
        let re = regex::Regex::new(pattern).map_err(|e| format!("Invalid regex: {}", e))?;
        let repo = self.repo.lock().map_err(|e| e.to_string())?;

        let batch_size = 200;
        let mut match_ids = Vec::new();
        let mut cursor_updated: i64 = 0;
        let mut cursor_id: i64 = 0;

        loop {
            // Only fetch id + content + updated_at — avoids loading large rows into memory
            let rows = repo
                .query_entries_regex_light(tag_mask, cursor_updated, cursor_id, batch_size + 1, sort_field, sort_order)
                .map_err(|e| e.to_string())?;

            let total = rows.len();
            let has_more = total > batch_size as usize;

            for (id, content, _) in &rows[..total.min(batch_size as usize)] {
                if re.is_match(content) {
                    match_ids.push(*id);
                }
            }

            if has_more {
                if let Some((last_id, _, last_updated)) = rows.get(batch_size as usize) {
                    cursor_updated = *last_updated;
                    cursor_id = *last_id;
                }
            }

            if !has_more || match_ids.len() > 5000 {
                break;
            }
        }

        // Now fetch full entries only for matches
        let mut all = Vec::with_capacity(match_ids.len().min(5000));
        for id in match_ids.into_iter().take(5000) {
            if let Ok(entry) = repo.get_entry(id) {
                all.push(Self::entry_to_response(entry));
            }
        }

        Ok(all)
    }

    /// Get single entry by ID
    pub fn get_entry_content(&self, id: i64) -> Result<String, String> {
        let repo = self.repo.lock().map_err(|e| e.to_string())?;
        let entry = repo.get_entry(id).map_err(|e| e.to_string())?;
        Ok(entry.content)
    }

    /// Delete entry and return whether an image was also removed.
    /// Uses split locking to avoid holding DB lock during file I/O.
    pub fn delete_entry(&self, id: i64) -> Result<bool, String> {
        let paths = {
            let repo = self.repo.lock().map_err(|e| e.to_string())?;
            repo.delete_entry(id).map_err(|e| e.to_string())?
        };

        let mut removed = false;
        let mut paths_to_remove = Vec::new();
        if let Some(path) = &paths.0 {
            paths_to_remove.push(path.clone());
            removed = true;
        }
        if let Some(path) = &paths.1 {
            paths_to_remove.push(path.clone());
            removed = true;
        }
        let freed = crate::service::image::remove_images(&self.app_data, &paths_to_remove);
        if freed > 0 {
            self.image_bytes_cache.fetch_sub(freed, Ordering::Relaxed);
        }
        Ok(removed)
    }

    /// Toggle favorite
    pub fn toggle_favorite(&self, id: i64, value: bool) -> Result<(), String> {
        let repo = self.repo.lock().map_err(|e| e.to_string())?;
        repo.toggle_favorite(id, value)
            .map_err(|e| e.to_string())
    }

    /// Set the no_auto_fav flag: once set, the entry will never be auto-favorited again.
    pub fn set_no_auto_fav(&self, id: i64, value: bool) -> Result<(), String> {
        let repo = self.repo.lock().map_err(|e| e.to_string())?;
        repo.set_no_auto_fav(id, value)
            .map_err(|e| e.to_string())
    }

    /// Bump updated_at to now (used by paste to move entry to top of list)
    pub fn touch_entry(&self, id: i64) -> Result<(), String> {
        let repo = self.repo.lock().map_err(|e| e.to_string())?;
        repo.update_timestamp(id).map_err(|e| e.to_string())
    }

    /// Increment copy_count for a user-triggered paste/copy from the list.
    /// Delegates auto-favorite decision to `try_auto_favorite_by_id` for
    /// consistent behavior with the clipboard-capture path.
    /// Returns true if the entry was auto-favorited.
    pub fn increment_copy_count(&self, id: i64, threshold: i64) -> Result<bool, String> {
        let repo = self.repo.lock().map_err(|e| e.to_string())?;
        let new_count = repo.increment_copy_count(id).map_err(|e| e.to_string())?;
        drop(repo);
        self.try_auto_favorite_by_id(id, new_count, threshold)
    }

    /// Auto-favorite an entry if copy_count meets threshold and no_auto_fav is not set.
    /// Shared between clipboard-capture and user-triggered paste/copy paths for consistency.
    pub fn try_auto_favorite_by_id(&self, id: i64, copy_count: i64, threshold: i64) -> Result<bool, String> {
        if threshold < 2 || copy_count < threshold {
            return Ok(false);
        }
        let repo = self.repo.lock().map_err(|e| e.to_string())?;
        let entry = repo.get_entry(id).map_err(|e| e.to_string())?;
        if entry.is_favorite {
            return Ok(false);
        }
        if repo.has_no_auto_fav_flag(id).map_err(|e| e.to_string())? {
            return Ok(false);
        }
        repo.toggle_favorite(id, true).map_err(|e| e.to_string())?;
        Ok(true)
    }

    /// Run cleanup of expired entries.
    /// Uses split locking to avoid holding DB lock during file I/O.
    pub fn cleanup(&self, retain_days: u32) -> Result<CleanupResult, String> {
        let (deleted, paths) = {
            let repo = self.repo.lock().map_err(|e| e.to_string())?;
            repo.cleanup(retain_days).map_err(|e| e.to_string())?
        };

        let freed = crate::service::image::remove_images(&self.app_data, &paths);
        if freed > 0 {
            self.image_bytes_cache.fetch_sub(freed, Ordering::Relaxed);
        }
        Ok(CleanupResult { deleted })
    }

    /// Clear all entries.
    /// Uses split locking to avoid holding DB lock during file I/O.
    pub fn clear_all(&self, keep_favorites: bool) -> Result<(), String> {
        let paths = {
            let repo = self.repo.lock().map_err(|e| e.to_string())?;
            repo.clear_all(keep_favorites).map_err(|e| e.to_string())?
        };

        let freed = crate::service::image::remove_images(&self.app_data, &paths);
        if freed > 0 {
            self.image_bytes_cache.fetch_sub(freed, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Get stats — uses cached image bytes (no stat() syscall per call)
    pub fn get_stats(&self) -> Result<StatsResponse, String> {
        let repo = self.repo.lock().map_err(|e| e.to_string())?;
        let stats = repo.get_stats().map_err(|e| e.to_string())?;
        let image_bytes = self.image_bytes_cache.load(Ordering::Relaxed);
        Ok(StatsResponse {
            count: stats.count,
            total_bytes: stats.total_bytes,
            image_bytes,
        })
    }

    /// Save a new clipboard entry or update existing (dedup).
    /// Uses three-phase locking to avoid holding the DB lock during file I/O.
    /// On dedup, increments copy_count and returns the updated count via the payload.
    /// `qr_text` is the decoded QR code content (empty if none detected).
    pub fn save_clipboard(
        &self,
        hash: &str,
        text: &str,
        tag_mask: i32,
        image_data: Option<&[u8]>,
        qr_text: &str,
    ) -> Result<model::ClipboardUpdatePayload, String> {
        let content_length = model::content_length(text);

        let (id, copy_count) = {
            let repo = self.repo.lock().map_err(|e| e.to_string())?;
            let (was_dedup, cc) = repo
                .upsert_dedup(hash, tag_mask, text, content_length)
                .map_err(|e| e.to_string())?;

            if was_dedup {
                let id = repo.find_id_by_hash(hash).map_err(|e| e.to_string())?;
                // Update qr_text on dedup (may have changed since last capture)
                if !qr_text.is_empty() {
                    let _ = repo.set_qr_text(id, qr_text);
                }
                (id, cc)
            } else {
                let id = repo
                    .insert_entry(hash, text, tag_mask, content_length, qr_text)
                    .map_err(|e| e.to_string())?;
                (id, 0)
            }
        };

        let image_result = if let Some(img_data) = image_data {
            crate::service::image::save_image_file(&self.app_data, id, img_data).ok()
        } else {
            None
        };

        if let Some((ref img_path, ref thumb_path, bytes)) = image_result {
            let repo = self.repo.lock().map_err(|e| e.to_string())?;
            let _ = repo.set_image_path(id, img_path);
            if !thumb_path.is_empty() {
                let _ = repo.set_thumb_path(id, thumb_path);
            }
            // Update cached image bytes directly from the known sizes
            // (no extra disk stat in the hot path).
            if bytes > 0 {
                self.image_bytes_cache.fetch_add(bytes, Ordering::Relaxed);
            }
        }

        let preview = crate::util::truncate(text, 40);
        Ok(model::ClipboardUpdatePayload {
            id,
            content_preview: preview,
            tag_mask,
            copy_count,
            auto_favorited: false,
            qr_text: qr_text.to_string(),
        })
    }

    /// Get the QR code text for an entry (empty string if none).
    pub fn get_entry_qr_text(&self, id: i64) -> Result<String, String> {
        let repo = self.repo.lock().map_err(|e| e.to_string())?;
        let entry = repo.get_entry(id).map_err(|e| e.to_string())?;
        Ok(entry.qr_text)
    }

    /// Get image dimensions (width, height) for window sizing
    pub fn get_entry_image_dimensions(&self, id: i64) -> Result<(f64, f64), String> {
        let repo = self.repo.lock().map_err(|e| e.to_string())?;
        let entry = repo.get_entry(id).map_err(|e| e.to_string())?;

        if entry.image_path.is_empty() {
            return Err("No image for this entry".to_string());
        }

        let full_path = Path::new(&self.app_data).join(&entry.image_path);
        let img = image::open(&full_path).map_err(|e| format!("Failed to open image: {}", e))?;
        Ok((img.width() as f64, img.height() as f64))
    }

    /// Get image file path for frontend display.
    /// Prefers the thumbnail (fast for list view); falls back to full image.
    /// Returns an absolute path that the frontend can use with file:// protocol.
    pub fn get_entry_image_path(&self, id: i64) -> Result<String, String> {
        let repo = self.repo.lock().map_err(|e| e.to_string())?;
        let entry = repo.get_entry(id).map_err(|e| e.to_string())?;

        if entry.image_path.is_empty() {
            return Err("No image for this entry".to_string());
        }

        // Prefer thumbnail for fast loading
        let path = if !entry.thumb_path.is_empty() {
            Path::new(&self.app_data).join(&entry.thumb_path)
        } else {
            Path::new(&self.app_data).join(&entry.image_path)
        };
        Ok(path.to_string_lossy().to_string())
    }

    /// Get full-resolution image file path (for image viewer window).
    pub fn get_entry_image_full_path(&self, id: i64) -> Result<String, String> {
        let repo = self.repo.lock().map_err(|e| e.to_string())?;
        let entry = repo.get_entry(id).map_err(|e| e.to_string())?;

        if entry.image_path.is_empty() {
            return Err("No image for this entry".to_string());
        }

        let full_path = Path::new(&self.app_data).join(&entry.image_path);
        Ok(full_path.to_string_lossy().to_string())
    }

    /// Get image entry IDs matching tag mask and search — delegates to repository.
    pub fn get_image_list(&self, tag_mask: i32, search: &str) -> Result<Vec<i64>, String> {
        let repo = self.repo.lock().map_err(|e| e.to_string())?;
        repo.get_image_list(tag_mask, search).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_service() -> (HistoryService, TempDir) {
        let dir = TempDir::new().unwrap();
        let svc = HistoryService::new(dir.path()).unwrap();
        (svc, dir)
    }

    #[test]
    fn test_get_entries_empty() {
        let (svc, _) = setup_service();
        let result = svc.get_entries("", 0, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert!(result.entries.is_empty());
        assert!(!result.has_more);
    }

    #[test]
    fn test_save_and_get() {
        let (svc, _) = setup_service();
        let payload = svc.save_clipboard("hash1", "hello world", model::TAG_TEXT, None, "").unwrap();
        assert!(payload.id > 0);

        let result = svc.get_entries("", 0, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].content, "hello world");
    }

    #[test]
    fn test_dedup_does_not_create_duplicate() {
        let (svc, _) = setup_service();
        svc.save_clipboard("dedup_hash", "content", model::TAG_TEXT, None, "").unwrap();
        svc.save_clipboard("dedup_hash", "updated content", model::TAG_TEXT, None, "").unwrap();

        let result = svc.get_entries("", 0, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert_eq!(result.entries.len(), 1, "dedup should not create duplicates");
        assert_eq!(result.entries[0].content, "updated content");
    }

    #[test]
    fn test_save_image_creates_file_and_path() {
        let (svc, dir) = setup_service();
        let img_data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]; // PNG header bytes
        let payload = svc.save_clipboard("img_hash", "", model::TAG_IMAGE, Some(&img_data), "").unwrap();
        assert!(payload.id > 0);

        // Verify image_path was set in DB
        let result = svc.get_entries("", 0, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert_eq!(result.entries.len(), 1);
        assert!(!result.entries[0].image_path.is_empty(), "image_path should be set");

        // Verify file exists on disk
        let full_path = dir.path().join(&result.entries[0].image_path);
        assert!(full_path.exists(), "image file should exist at {:?}", full_path);
    }

    #[test]
    fn test_different_images_get_different_hashes() {
        let (svc, _dir) = setup_service();
        let img_a = vec![0x89, 0x50, 0x4E, 0x47, 0x01, 0x02, 0x03, 0x04];
        let img_b = vec![0x89, 0x50, 0x4E, 0x47, 0xAA, 0xBB, 0xCC, 0xDD];

        let p1 = svc.save_clipboard("hash_a", "", model::TAG_IMAGE, Some(&img_a), "").unwrap();
        let p2 = svc.save_clipboard("hash_b", "", model::TAG_IMAGE, Some(&img_b), "").unwrap();

        // Different hashes → different entries
        assert_ne!(p1.id, p2.id, "different image data should create different entries");

        let result = svc.get_entries("", 0, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert_eq!(result.entries.len(), 2, "should have 2 separate image entries");
    }

    #[test]
    fn test_thumbnail_generation() {
        let (svc, dir) = setup_service();
        // Create a larger test image (500x500)
        let img = image::RgbImage::new(500, 500);
        let img_data = {
            let mut buf = Vec::new();
            let mut cursor = std::io::Cursor::new(&mut buf);
            img.write_to(&mut cursor, image::ImageFormat::Png).unwrap();
            buf
        };
        let img_size = img_data.len();

        let _payload = svc.save_clipboard("thumb_test", "", model::TAG_IMAGE, Some(&img_data), "").unwrap();

        // Verify thumbnail was created
        let result = svc.get_entries("", 0, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert_eq!(result.entries.len(), 1);
        assert!(!result.entries[0].thumb_path.is_empty(), "thumb_path should be set");

        // Verify thumbnail file exists and is smaller than original
        let thumb_full = dir.path().join(&result.entries[0].thumb_path);
        assert!(thumb_full.exists(), "thumbnail file should exist");
        let thumb_size = std::fs::metadata(&thumb_full).unwrap().len();
        assert!(thumb_size < img_size as u64, "thumbnail ({}) should be smaller than original ({})", thumb_size, img_size);
    }

    #[test]
    fn test_get_entry_image_dimensions() {
        let (svc, dir) = setup_service();
        // Create a 100x50 RGB image and save as PNG
        let img = image::RgbImage::new(100, 50);
        let img_path = dir.path().join("test_img.png");
        img.save(&img_path).unwrap();

        // Insert entry with image_path pointing to the file (relative to app_data)
        let payload = svc.save_clipboard("dim_hash", "", model::TAG_IMAGE, None, "").unwrap();
        // Manually set image_path since save_clipboard with None won't set it
        {
            let repo = svc.repo.lock().unwrap();
            repo.set_image_path(payload.id, "test_img.png").unwrap();
        }

        // This will fail because the path is relative to app_data, not the temp dir directly
        // For a proper test we'd need the images subdirectory, so let's just verify the method exists
        // and works when given a valid path
        let result = svc.get_entry_image_dimensions(payload.id);
        // The path "test_img.png" is relative to app_data (dir.path()), so it should find it
        assert!(result.is_ok(), "should find image at {:?}", result);
        let (w, h) = result.unwrap();
        assert_eq!(w, 100.0);
        assert_eq!(h, 50.0);
    }

    #[test]
    fn test_delete_entry() {
        let (svc, _) = setup_service();
        let payload = svc.save_clipboard("del_hash", "delete me", model::TAG_TEXT, None, "").unwrap();

        let had_image = svc.delete_entry(payload.id).unwrap();
        assert!(!had_image);

        let result = svc.get_entries("", 0, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert_eq!(result.entries.len(), 0);
    }

    #[test]
    fn test_toggle_favorite() {
        let (svc, _) = setup_service();
        let payload = svc.save_clipboard("fav_hash", "favorite", 0, None, "").unwrap();

        svc.toggle_favorite(payload.id, true).unwrap();
        let result = svc.get_entries("", model::TAG_FAVORITE, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert_eq!(result.entries.len(), 1);
    }

    #[test]
    fn test_search() {
        let (svc, _) = setup_service();
        svc.save_clipboard("h1", "find me", model::TAG_TEXT, None, "").unwrap();
        svc.save_clipboard("h2", "dont match", model::TAG_TEXT, None, "").unwrap();

        let result = svc.get_entries("find", 0, 0, 0, 20, "updated_at", "DESC").unwrap();
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].content, "find me");
    }

    #[test]
    fn test_cleanup() {
        let (svc, dir) = setup_service();
        svc.save_clipboard("old_hash", "old", 0, None, "").unwrap();

        // Manually update timestamp to be old via repo
        {
            let repo_path = dir.path().join("clipboard.db");
            let repo = Repository::new(&repo_path).unwrap();
            repo.db
                .execute(
                    "UPDATE entries SET updated_at = 1000 WHERE content_hash = 'old_hash'",
                    [],
                )
                .unwrap();
        }

        let result = svc.cleanup(1).unwrap();
        assert_eq!(result.deleted, 1);
    }

    #[test]
    fn test_stats() {
        let (svc, _) = setup_service();
        svc.save_clipboard("h1", "hello", model::TAG_TEXT, None, "").unwrap();
        svc.save_clipboard("h2", "world", model::TAG_TEXT, None, "").unwrap();

        let stats = svc.get_stats().unwrap();
        assert_eq!(stats.count, 2);
    }
}
