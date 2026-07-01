use anyhow::Result;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// ImageStore manages clipboard image files on disk.
pub struct ImageStore {
    data_dir: PathBuf,
}

impl ImageStore {
    /// Create a new ImageStore rooted at `{data_dir}`.
    pub fn new(data_dir: &Path) -> Self {
        Self { data_dir: data_dir.to_path_buf() }
    }

    /// Save raw PNG bytes to disk. Returns relative path `images/YYYY-MM-DD/{uuid}.png`.
    pub fn save_png(&self, png_data: &[u8]) -> Result<String> {
        let date_dir = chrono_dir();
        let dir = self.data_dir.join("images").join(&date_dir);
        std::fs::create_dir_all(&dir)?;

        let filename = format!("{}.png", Uuid::new_v4());
        let relative = format!("images/{}/{}", date_dir, filename);
        std::fs::write(dir.join(&filename), png_data)?;
        Ok(relative)
    }

    /// Load PNG bytes from a relative path (e.g. `images/2026-06-02/uuid.png`).
    pub fn load_png(&self, relative_path: &str) -> Result<Vec<u8>> {
        let full = self.data_dir.join(relative_path);
        Ok(std::fs::read(&full)?)
    }

    /// Delete a single image file by relative path.
    pub fn delete(&self, relative_path: &str) -> Result<()> {
        let full = self.data_dir.join(relative_path);
        if full.exists() {
            std::fs::remove_file(&full)?;
            // Try to clean up empty parent dir.
            if let Some(parent) = full.parent() {
                if parent.read_dir().map(|mut it| it.next().is_none()).unwrap_or(false) {
                    let _ = std::fs::remove_dir(parent);
                }
            }
        }
        Ok(())
    }

    /// Delete all image files and directories.
    pub fn delete_all(&self) -> Result<()> {
        let images_dir = self.data_dir.join("images");
        if images_dir.exists() {
            std::fs::remove_dir_all(&images_dir)?;
        }
        Ok(())
    }
}

fn chrono_dir() -> String {
    // Use current local date YYYY-MM-DD.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    // Days since epoch.
    let days = secs / 86400;
    // Quick-and-dirty date without pulling in chrono.
    let (y, m, d) = days_to_date(days as i64 + 719468); // days from 0000-01-01
    format!("{:04}-{:02}-{:02}", y, m, d)
}

/// Convert days since 1970-01-01 to (year, month, day).
/// Algorithm from Howard Hinnant, via https://howardhinnant.github.io/date_algorithms.html
fn days_to_date(days_since_1970: i64) -> (i64, u32, u32) {
    let z = days_since_1970 + 719468; // shift to March 1, year 0
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
    let mp = (5 * doy + 2) / 153; // month phase [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // day [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // month [1, 12]
    let y = if m <= 2 { y + 1 } else { y }; // year
    (y, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_days_to_date() {
        let tests = [
            (0, (1970, 1, 1)),
            (365, (1971, 1, 1)),
        ];
        for (days, (ey, em, ed)) in tests {
            let (y, m, d) = days_to_date(days);
            assert_eq!((y, m, d), (ey, em, ed), "days={}", days);
        }
    }

    #[test]
    fn test_chrono_dir_format() {
        let dir = chrono_dir();
        // Should be YYYY-MM-DD
        assert_eq!(dir.len(), 10);
        assert_eq!(dir.as_bytes()[4], b'-');
        assert_eq!(dir.as_bytes()[7], b'-');
    }

    #[test]
    fn test_save_and_delete() {
        let dir = tempfile::tempdir().unwrap();
        let store = ImageStore::new(dir.path());
        let png_data = b"fake_png_bytes";

        let path = store.save_png(png_data).unwrap();
        assert!(path.starts_with("images/"));

        let loaded = store.load_png(&path).unwrap();
        assert_eq!(loaded, png_data);

        store.delete(&path).unwrap();
        assert!(store.load_png(&path).is_err());
    }
}
