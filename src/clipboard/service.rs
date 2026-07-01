//! ClipboardService — orchestrates capture → store → queue pipeline.
//!
//! Owns the mutable state (FiloStack, SelfWriteTracker) so `App` can
//! delegate clipboard processing through a single call. Repository and
//! ImageStore are passed as references since `App` also needs them.

use anyhow::Result;

use crate::clipboard::capture::CapturedData;
use crate::clipboard::source::get_clipboard_source;
use crate::filostack::service::FiloStackService;
use crate::storage::image_store::ImageStore;
use crate::storage::repository::{self, Repository};
use crate::util::hash::{sha256_hex, sha256_hex_bytes};
use crate::util::tracker::SelfWriteTracker;

/// Clipboard processing service.
pub struct ClipboardService {
    filo_stack: FiloStackService,
    tracker: SelfWriteTracker,
}

impl ClipboardService {
    /// Create a new service with a pre-configured FiloStack.
    pub fn new(filo_stack: FiloStackService) -> Self {
        Self {
            filo_stack,
            tracker: SelfWriteTracker::new(),
        }
    }

    /// Process captured clipboard data: dedup, store, queue.
    /// Returns the entry ID if stored, `None` if skipped (self-write or empty).
    pub fn handle(
        &mut self,
        data: &CapturedData,
        repo: &Repository,
        image_store: &ImageStore,
    ) -> Result<Option<i64>> {
        // 1. Self-write guard
        if self.tracker.is_self_write(&data.content_hash) {
            return Ok(None);
        }

        // 2. Auto-exit queue mode on image / file
        self.filo_stack.auto_exit_if_needed(data.tag_mask);

        // 3. Source tracking
        let source = get_clipboard_source();

        // 4. Upsert entry (dedup by content_hash)
        let eid = repo.upsert_entry(
            &data.content_hash,
            &source.exe,
            &source.title,
            data.tag_mask,
            data.content_length,
        )?;
        if eid == 0 {
            return Ok(None);
        }

        // 5. Upsert formats
        if let Some(text) = &data.primary_text {
            let h = sha256_hex(text);
            repo.upsert_format(
                eid,
                repository::format::CF_UNICODETEXT,
                Some(text),
                None,
                &h,
            )?;
        }
        if let Some(png) = &data.image_png {
            if let Ok(path) = image_store.save_png(png) {
                let h = sha256_hex_bytes(png);
                repo.upsert_format(
                    eid,
                    repository::format::CF_DIB,
                    None,
                    Some(&path),
                    &h,
                )?;
            }
        }
        if !data.file_paths.is_empty() {
            let joined = data.file_paths.join("\n");
            let h = sha256_hex(&joined);
            repo.upsert_format(
                eid,
                repository::format::CF_HDROP,
                Some(&joined),
                None,
                &h,
            )?;
        }

        // 6. Queue for FiloStack (text only)
        if let Some(text) = &data.primary_text {
            self.filo_stack.push(text);
        }

        Ok(Some(eid))
    }

    // ── Accessors ──────────────────────────────────────────────

    pub fn filo_stack(&self) -> &FiloStackService {
        &self.filo_stack
    }

    pub fn filo_stack_mut(&mut self) -> &mut FiloStackService {
        &mut self.filo_stack
    }

    pub fn tracker(&self) -> &SelfWriteTracker {
        &self.tracker
    }

    pub fn tracker_mut(&mut self) -> &mut SelfWriteTracker {
        &mut self.tracker
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::capture::tag;
    use crate::storage::db::{self, DbConnection};
    use std::sync::Mutex;

    fn setup_repo() -> Repository {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        db::migrate(&conn).unwrap();
        Repository::new(DbConnection {
            conn: Mutex::new(conn),
        })
    }

    #[test]
    fn test_handle_skips_self_write() {
        let repo = setup_repo();
        let image_store = ImageStore::new(std::path::Path::new("."));
        let mut svc = ClipboardService::new(FiloStackService::new());

        let data = CapturedData {
            primary_text: Some("hello".into()),
            image_png: None,
            file_paths: vec![],
            content_hash: "will_be_skipped".into(),
            tag_mask: tag::TEXT,
            content_length: 5,
        };

        // Mark as self-write first
        svc.tracker_mut().mark(data.content_hash.clone());
        let result = svc.handle(&data, &repo, &image_store).unwrap();
        assert!(result.is_none(), "self-write should be skipped");
    }

    #[test]
    fn test_handle_stores_entry() {
        let repo = setup_repo();
        let dir = tempfile::tempdir().unwrap();
        let image_store = ImageStore::new(dir.path());
        let mut svc = ClipboardService::new(FiloStackService::new());

        let data = CapturedData {
            primary_text: Some("hello world".into()),
            image_png: None,
            file_paths: vec![],
            content_hash: "hash_abc".into(),
            tag_mask: tag::TEXT,
            content_length: 11,
        };

        let eid = svc.handle(&data, &repo, &image_store).unwrap().expect("should store");
        assert!(eid > 0);

        // Verify in DB
        assert!(repo.exists_by_hash("hash_abc").unwrap());
        let content = repo.get_format_content(eid, repository::format::CF_UNICODETEXT).unwrap();
        assert_eq!(content, Some("hello world".into()));
    }
}
