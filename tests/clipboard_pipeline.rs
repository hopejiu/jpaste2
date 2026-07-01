//! Clipboard pipeline integration tests.
//!
//! Tests ClipboardService::handle with real Repository and ImageStore.

mod common;

use jpastev2::clipboard::capture::{tag, CapturedData};
use jpastev2::clipboard::service::ClipboardService;
use jpastev2::filostack::service::FiloStackService;
use jpastev2::storage::image_store::ImageStore;

#[test]
fn test_pipeline_stores_text_entry() {
    let repo = common::temp_repo();
    let dir = tempfile::tempdir().unwrap();
    let image_store = ImageStore::new(dir.path());
    let mut svc = ClipboardService::new(FiloStackService::new());

    let data = CapturedData {
        primary_text: Some("hello clipboard pipeline".into()),
        image_png: None,
        file_paths: vec![],
        content_hash: "pipeline_hash_1".into(),
        tag_mask: tag::TEXT,
        content_length: 22,
    };

    let eid = svc.handle(&data, &repo, &image_store).unwrap().expect("should store");
    assert!(eid > 0, "entry id should be positive");
    assert!(repo.exists_by_hash("pipeline_hash_1").unwrap(), "entry should exist");
}

#[test]
fn test_pipeline_dedup_same_hash() {
    let repo = common::temp_repo();
    let dir = tempfile::tempdir().unwrap();
    let image_store = ImageStore::new(dir.path());
    let mut svc = ClipboardService::new(FiloStackService::new());

    let data = CapturedData {
        primary_text: Some("dedup test".into()),
        image_png: None,
        file_paths: vec![],
        content_hash: "dedup_hash".into(),
        tag_mask: tag::TEXT,
        content_length: 9,
    };

    let eid1 = svc.handle(&data, &repo, &image_store).unwrap();
    let eid2 = svc.handle(&data, &repo, &image_store).unwrap();
    assert_eq!(eid1, eid2, "same hash should return same entry id");
}

#[test]
fn test_pipeline_skips_self_write() {
    let repo = common::temp_repo();
    let dir = tempfile::tempdir().unwrap();
    let image_store = ImageStore::new(dir.path());
    let mut svc = ClipboardService::new(FiloStackService::new());

    // Pre-mark the hash
    svc.tracker_mut().mark("self_write_hash".into());

    let data = CapturedData {
        primary_text: Some("self-written content".into()),
        image_png: None,
        file_paths: vec![],
        content_hash: "self_write_hash".into(),
        tag_mask: tag::TEXT,
        content_length: 19,
    };

    let result = svc.handle(&data, &repo, &image_store).unwrap();
    assert!(result.is_none(), "self-write should be skipped");
}

#[test]
fn test_pipeline_stores_all_formats() {
    let repo = common::temp_repo();
    let dir = tempfile::tempdir().unwrap();
    let image_store = ImageStore::new(dir.path());
    let mut svc = ClipboardService::new(FiloStackService::new());

    let data = CapturedData {
        primary_text: Some("text".into()),
        image_png: Some(vec![0x89, 0x50, 0x4E, 0x47]), // minimal PNG header
        file_paths: vec!["C:\\file1.txt".into(), "C:\\file2.txt".into()],
        content_hash: "multi_format".into(),
        tag_mask: tag::TEXT | tag::IMAGE | tag::FILE,
        content_length: 4,
    };

    let eid = svc.handle(&data, &repo, &image_store).unwrap().expect("should store");
    assert!(eid > 0);

    // Verify text format
    let text = repo.get_format_content(eid, 13).unwrap();
    assert_eq!(text, Some("text".into()));

    // Verify image format has a file path by checking the image_path
    // in the entry returned from get_history
    let entries = repo
        .get_history(0, "", 0, None, "updated_at", "desc", 10)
        .unwrap();
    let entry = entries.iter().find(|e| e.id == eid).expect("entry should exist");
    assert_eq!(entry.tag_mask & 4, 4, "entry should have image tag");
    let ip = entry.image_path.as_deref().expect("image path should be set");
    assert!(ip.starts_with("images/"), "image path should start with images/");
}

#[test]
fn test_pipeline_auto_exits_queue_on_image() {
    let repo = common::temp_repo();
    let dir = tempfile::tempdir().unwrap();
    let image_store = ImageStore::new(dir.path());
    let mut svc = ClipboardService::new({
        let mut fs = FiloStackService::new();
        fs.set_mode("queue");
        fs
    });

    // Image capture should auto-exit queue
    let data = CapturedData {
        primary_text: None,
        image_png: Some(vec![0x89, 0x50, 0x4E, 0x47]),
        file_paths: vec![],
        content_hash: "img_no_queue".into(),
        tag_mask: tag::IMAGE,
        content_length: 0,
    };

    let _ = svc.handle(&data, &repo, &image_store).unwrap();
    assert_eq!(svc.filo_stack().mode(), "normal", "queue mode should auto-exit on image");
}
