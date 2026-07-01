//! ImageStore integration tests.
//!
//! Tests save → load → delete lifecycle with real filesystem.

use jpastev2::storage::image_store::ImageStore;

#[test]
fn test_image_save_load_delete() {
    let dir = tempfile::tempdir().unwrap();
    let store = ImageStore::new(dir.path());

    let png_data = b"\x89PNG\r\n\x1a\nfake_png";
    let path = store.save_png(png_data).unwrap();
    assert!(path.starts_with("images/"));

    let loaded = store.load_png(&path).unwrap();
    assert_eq!(loaded, png_data);

    store.delete(&path).unwrap();
    assert!(store.load_png(&path).is_err());
}

#[test]
fn test_image_delete_all() {
    let dir = tempfile::tempdir().unwrap();
    let store = ImageStore::new(dir.path());

    store.save_png(b"data1").unwrap();
    store.save_png(b"data2").unwrap();

    store.delete_all().unwrap();

    // After delete_all, the images/ directory should not exist
    let images_dir = dir.path().join("images");
    assert!(!images_dir.exists());
}

#[test]
fn test_image_save_creates_date_subdir() {
    let dir = tempfile::tempdir().unwrap();
    let store = ImageStore::new(dir.path());

    let path = store.save_png(b"test").unwrap();
    // Path should be "images/YYYY-MM-DD/uuid.png"
    let parts: Vec<&str> = path.split('/').collect();
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0], "images");

    // Date part should be valid YYYY-MM-DD
    let date_part = parts[1];
    assert_eq!(date_part.len(), 10);
    assert_eq!(date_part.as_bytes()[4], b'-');
    assert_eq!(date_part.as_bytes()[7], b'-');
}

#[test]
fn test_image_delete_removes_empty_dir() {
    let dir = tempfile::tempdir().unwrap();
    let store = ImageStore::new(dir.path());

    let path = store.save_png(b"cleanup_test").unwrap();
    store.delete(&path).unwrap();

    // The date directory should have been cleaned up
    let full_path = dir.path().join(&path);
    let parent = full_path.parent().unwrap();
    assert!(!parent.exists() || parent.read_dir().unwrap().next().is_none());
}
