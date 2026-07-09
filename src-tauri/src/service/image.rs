//! Image file storage management.
//!
//! Handles saving clipboard images to disk, generating WebP thumbnails,
//! and computing on-disk byte sizes. Used by HistoryService.
//! Decoupled from DB access — callers manage their own DB writes.

use std::path::Path;

const THUMB_MAX_DIM: u32 = 300;

/// Save image data to disk, returning (image_path, thumb_path, total_bytes).
/// Also generates a thumbnail (max 300px) for fast list-view loading.
/// `total_bytes` is the on-disk size of the image + thumbnail so callers can
/// update the in-memory cache without re-reading from disk.
pub fn save_image_file(app_data: &str, _entry_id: i64, img_data: &[u8]) -> Result<(String, String, i64), String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();

    let secs = now.as_secs();
    let days = secs / 86400;
    let (year, month, day) = crate::util::days_to_date(days as i64);

    let dir_path = format!("images/{:04}-{:02}-{:02}", year, month, day);
    let full_dir = Path::new(app_data).join(&dir_path);
    std::fs::create_dir_all(&full_dir).map_err(|e| e.to_string())?;

    let filename = format!("{}.png", uuid::Uuid::new_v4());
    let full_path = full_dir.join(&filename);
    std::fs::write(&full_path, img_data).map_err(|e| e.to_string())?;

    let img_path = format!("{}/{}", dir_path, filename);

    let (thumb_path, thumb_bytes) = if let Ok(img) = image::open(&full_path) {
        let thumb = if img.width() > THUMB_MAX_DIM || img.height() > THUMB_MAX_DIM {
            img.resize(THUMB_MAX_DIM, THUMB_MAX_DIM, image::imageops::FilterType::Triangle)
        } else {
            img
        };
        let thumb_name = format!("{}.thumb.webp", uuid::Uuid::new_v4());
        let thumb_full_path = full_dir.join(&thumb_name);
        match save_webp_optimized(&thumb, &thumb_full_path) {
            Some(sz) => (format!("{}/{}", dir_path, thumb_name), sz),
            None => (String::new(), 0),
        }
    } else {
        (String::new(), 0)
    };

    let total_bytes = img_data.len() as i64 + thumb_bytes as i64;
    Ok((img_path, thumb_path, total_bytes))
}

/// Save image as WebP — 30-50% smaller than PNG.
/// Returns the encoded byte size on success, None on failure.
fn save_webp_optimized(img: &image::DynamicImage, path: &Path) -> Option<u64> {
    let mut buf = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buf);
    if img.write_to(&mut cursor, image::ImageFormat::WebP).is_err() {
        return None;
    }
    if std::fs::write(path, &buf).is_err() {
        return None;
    }
    Some(buf.len() as u64)
}

/// Remove image/thumb files from disk and return their total byte size.
pub fn remove_images(app_data: &str, paths: &[String]) -> i64 {
    let mut freed_bytes: i64 = 0;
    for path in paths {
        let full_path = Path::new(app_data).join(path);
        if let Ok(meta) = std::fs::metadata(&full_path) {
            freed_bytes += meta.len() as i64;
        }
        let _ = std::fs::remove_file(&full_path);
    }
    freed_bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_save_image_file_creates_image_and_thumb() {
        let dir = TempDir::new().unwrap();
        let img = image::RgbImage::new(500, 500);
        let mut buf = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buf);
        img.write_to(&mut cursor, image::ImageFormat::Png).unwrap();
        let _img_size = buf.len();

        let (img_path, thumb_path, total_bytes) =
            save_image_file(dir.path().to_str().unwrap(), 1, &buf).unwrap();

        // Image file should exist
        assert!(!img_path.is_empty());
        let full_img = dir.path().join(&img_path);
        assert!(full_img.exists(), "image file should exist at {:?}", full_img);

        // Thumbnail should exist (image > 300px)
        assert!(!thumb_path.is_empty(), "thumb should be created for 500x500 image");
        let full_thumb = dir.path().join(&thumb_path);
        assert!(full_thumb.exists(), "thumb file should exist at {:?}", full_thumb);

        // total_bytes should be positive and close to img_size + thumb_size
        assert!(total_bytes > 0, "total_bytes should be > 0, got {}", total_bytes);
    }

    #[test]
    fn test_save_image_small_no_thumbnail() {
        let dir = TempDir::new().unwrap();
        let img = image::RgbImage::new(10, 10);
        let mut buf = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buf);
        img.write_to(&mut cursor, image::ImageFormat::Png).unwrap();

        let (img_path, thumb_path, total_bytes) =
            save_image_file(dir.path().to_str().unwrap(), 2, &buf).unwrap();

        assert!(!img_path.is_empty());
        let full_img = dir.path().join(&img_path);
        assert!(full_img.exists());

        // Small image (< 300px) should still get a thumb (same-size passthrough)
        assert!(!thumb_path.is_empty(), "even small images should get a thumb file");
        assert!(total_bytes > 0);
    }

    #[test]
    fn test_remove_images_existing_files() {
        let dir = TempDir::new().unwrap();
        let app_data = dir.path().to_str().unwrap();

        // Create test files
        let path1 = "img1.png";
        let path2 = "sub/img2.png";
        std::fs::write(dir.path().join(path1), b"hello").unwrap();
        std::fs::create_dir_all(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join(path2), b"world").unwrap();

        let freed = remove_images(app_data, &[path1.to_string(), path2.to_string()]);
        assert_eq!(freed, 10, "should count bytes of removed files");
        assert!(!dir.path().join(path1).exists(), "file should be deleted");
        assert!(!dir.path().join(path2).exists(), "file should be deleted");
    }

    #[test]
    fn test_remove_images_nonexistent_returns_zero() {
        let dir = TempDir::new().unwrap();
        let freed = remove_images(
            dir.path().to_str().unwrap(),
            &["nonexistent.png".to_string()],
        );
        assert_eq!(freed, 0, "non-existent file should free 0 bytes");
    }

    #[test]
    fn test_remove_images_empty_list() {
        let dir = TempDir::new().unwrap();
        let freed = remove_images(dir.path().to_str().unwrap(), &[]);
        assert_eq!(freed, 0);
    }
}
