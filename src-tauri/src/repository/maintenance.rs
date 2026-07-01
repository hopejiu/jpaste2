//! Cleanup, clear, and statistics operations.

use crate::model;
use crate::repository::Repository;
use rusqlite::{params, Result as SqlResult};

impl Repository {
    /// Delete entries older than retain_days, respecting favorites
    /// Returns (deleted_count, list_of_file_paths_to_remove)
    /// Uses SQLite RETURNING clause (3.35+) to avoid separate SELECT.
    pub fn cleanup(&self, retain_days: u32) -> SqlResult<(u64, Vec<String>)> {
        // Use bound parameter instead of format! for cutoff
        let sql = "DELETE FROM entries WHERE updated_at < (CAST(strftime('%s', 'now') AS INTEGER) - CAST(?1 AS INTEGER) * 86400) * 1000 AND is_favorite = 0 RETURNING image_path, thumb_path";

        let pairs = self.collect_path_pairs(sql, params![retain_days])?;
        let mut paths = Vec::new();
        let mut count = 0u64;
        for (img, thumb) in pairs {
            count += 1;
            if !img.is_empty() {
                paths.push(img);
            }
            if !thumb.is_empty() {
                paths.push(thumb);
            }
        }

        log::info!(
            "repository::cleanup: deleted {} entries, {} files to remove",
            count,
            paths.len()
        );
        Ok((count, paths))
    }

    /// Clear all entries (optionally keep favorites).
    /// Returns list of file paths (images + thumbs) to remove.
    pub fn clear_all(&self, keep_favorites: bool) -> SqlResult<Vec<String>> {
        let where_clause = if keep_favorites {
            " WHERE is_favorite = 0"
        } else {
            ""
        };
        let pairs = self.collect_path_pairs(&format!("SELECT image_path, thumb_path FROM entries{}", where_clause), &[])?;
        let paths = Self::flatten_paths(pairs);

        if keep_favorites {
            self.db
                .execute("DELETE FROM entries WHERE is_favorite = 0", [])?;
        } else {
            self.db.execute("DELETE FROM entries", [])?;
        };

        Ok(paths)
    }

    /// Run `SELECT image_path, thumb_path ...` and return each row as a
    /// (image_path, thumb_path) pair, skipping rows where both are empty.
    /// Single source of truth for image/thumb collection used by cleanup,
    /// clear_all, and get_image_storage_bytes.
    fn collect_path_pairs(
        &self,
        sql: &str,
        params: &[&dyn rusqlite::ToSql],
    ) -> SqlResult<Vec<(String, String)>> {
        let mut stmt = self.db.prepare(sql)?;
        let rows = stmt.query_map(params, |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut result = Vec::new();
        for row in rows {
            let (img, thumb) = row?;
            if img.is_empty() && thumb.is_empty() {
                continue;
            }
            result.push((img, thumb));
        }
        Ok(result)
    }

    /// Flatten (image_path, thumb_path) pairs into a single path list,
    /// dropping empty components.
    fn flatten_paths(pairs: Vec<(String, String)>) -> Vec<String> {
        let mut out = Vec::new();
        for (img, thumb) in pairs {
            if !img.is_empty() {
                out.push(img);
            }
            if !thumb.is_empty() {
                out.push(thumb);
            }
        }
        out
    }

    /// Get statistics. total_bytes = text content length on disk.
    /// Image file sizes are tracked separately via get_image_storage_bytes().
    pub fn get_stats(&self) -> SqlResult<model::Stats> {
        let (count, total_bytes): (i64, i64) = self.db.query_row(
            "SELECT (SELECT COUNT(*) FROM entries), COALESCE((SELECT SUM(LENGTH(content)) FROM entries), 0)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        Ok(model::Stats {
            count,
            total_bytes,
            image_bytes: 0,
        })
    }

    /// Get total image file size on disk (uses partial index for efficiency).
    /// Collects paths first, then does file I/O outside the query iteration.
    pub fn get_image_storage_bytes(&self, app_data: &std::path::Path) -> SqlResult<i64> {
        let sql = "SELECT image_path, thumb_path FROM entries WHERE image_path != ''";
        // Collect paths first to minimize time spent in query iteration
        let paths = self.collect_path_pairs(sql, &[])?;

        // File I/O outside the query map iterator
        let mut total: i64 = 0;
        for (img, thumb) in paths {
            if !img.is_empty() {
                let full = app_data.join(&img);
                if let Ok(meta) = std::fs::metadata(&full) {
                    total += meta.len() as i64;
                }
            }
            if !thumb.is_empty() {
                let full = app_data.join(&thumb);
                if let Ok(meta) = std::fs::metadata(&full) {
                    total += meta.len() as i64;
                }
            }
        }
        Ok(total)
    }
}
