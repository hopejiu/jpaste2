use anyhow::Result;
use rusqlite::{params};

use crate::storage::db::DbConnection;

/// Win32 clipboard format constants.
pub mod format {
    pub const CF_UNICODETEXT: i32 = 13;
    pub const CF_HDROP: i32 = 15;
    pub const CF_DIB: i32 = 8;
    pub const CF_DIBV5: i32 = 17;
}

/// An entry row from clipboard_entry.
#[derive(Debug, Clone)]
pub struct Entry {
    pub id: i64,
    pub content_hash: String,
    pub content: String,
    pub source_exe: String,
    pub source_title: String,
    pub tag_mask: i32,
    pub is_favorite: bool,
    pub content_length: i32,
    pub created_at: String,
    pub updated_at: String,
    pub image_path: Option<String>,
}

/// Repository owns ALL SQLite queries.
pub struct Repository {
    pub db: DbConnection,
}

impl Repository {
    pub fn new(db: DbConnection) -> Self {
        Self { db }
    }

    // ── Queries ────────────────────────────────────────────────

    /// Upsert an entry by content_hash (deduplication).
    pub fn upsert_entry(
        &self,
        content_hash: &str,
        source_exe: &str,
        source_title: &str,
        tag_mask: i32,
        content_length: i32,
    ) -> Result<i64> {
        let conn = self.db.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO clipboard_entry (content_hash, source_exe, source_title, tag_mask, content_length)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(content_hash) DO UPDATE SET
               updated_at = strftime('%Y-%m-%dT%H:%M:%f', 'now'),
               source_exe = excluded.source_exe,
               source_title = excluded.source_title,
               tag_mask = excluded.tag_mask,
               content_length = excluded.content_length",
            params![content_hash, source_exe, source_title, tag_mask, content_length],
        )?;

        // Return the entry ID (either newly inserted or existing).
        let id: i64 = conn.query_row(
            "SELECT id FROM clipboard_entry WHERE content_hash = ?1",
            params![content_hash],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    /// Upsert a format for an entry.
    pub fn upsert_format(
        &self,
        entry_id: i64,
        format_type: i32,
        content: Option<&str>,
        file_path: Option<&str>,
        format_hash: &str,
    ) -> Result<()> {
        let conn = self.db.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO clipboard_format (entry_id, format_type, content, file_path, format_hash)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(entry_id, format_type) DO UPDATE SET
               content = COALESCE(excluded.content, content),
               file_path = COALESCE(excluded.file_path, file_path),
               format_hash = excluded.format_hash",
            params![entry_id, format_type, content, file_path, format_hash],
        )?;
        Ok(())
    }

    /// Cursor-paginated history query.
    pub fn get_history(
        &self,
        tag_mask: i32,
        cursor_updated: &str,
        cursor_id: i64,
        search: Option<&str>,
        sort_field: &str,
        sort_order: &str,
        limit: i64,
    ) -> Result<Vec<Entry>> {
        let conn = self.db.conn.lock().unwrap();
        let tag_clause = if tag_mask == 0 {
            String::new()
        } else {
            format!("AND (e.tag_mask & {tag_mask}) != 0")
        };

        let search_clause = match search {
            Some(s) if !s.is_empty() => format!(
                "AND e.id IN (SELECT entry_id FROM clipboard_format WHERE content LIKE '%{}%')",
                s.replace('\'', "''")
            ),
            _ => String::new(),
        };

        let cursor_op = if sort_order == "desc" { "<" } else { ">" };
        let order = format!("ORDER BY e.{} {}", sort_field, sort_order);

        let sql = format!(
            "SELECT e.id, e.content_hash, e.source_exe, e.source_title,
                    e.tag_mask, e.is_favorite, e.content_length,
                    e.created_at, e.updated_at,
                    f.content
             FROM clipboard_entry e
             LEFT JOIN clipboard_format f ON f.entry_id = e.id AND f.format_type = 13
             WHERE (e.updated_at, e.id) {cursor_op} (?1, ?2) {tag_clause} {search_clause}
             {order}
             LIMIT ?3"
        );

        // If no cursor, use boundary values (max for desc, min for asc).
        let (cu, ci) = if cursor_updated.is_empty() {
            if sort_order == "desc" {
                ("9999-12-31T23:59:59.999", i64::MAX)
            } else {
                ("0000-01-01T00:00:00.000", 0)
            }
        } else {
            (cursor_updated, cursor_id)
        };

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![cu, ci, limit], |row| {
            Ok(Entry {
                id: row.get(0)?,
                content_hash: row.get(1)?,
                content: row.get::<_, Option<String>>(9).unwrap_or_default().unwrap_or_default(),
                source_exe: row.get(2)?,
                source_title: row.get(3)?,
                tag_mask: row.get(4)?,
                is_favorite: row.get::<_, i32>(5)? != 0,
                content_length: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
                image_path: None,
            })
        })?;

        let mut entries: Vec<Entry> = Vec::new();
        for row in rows {
            entries.push(row?);
        }

        // Load image paths for image entries.
        for entry in &mut entries {
            if entry.tag_mask & 4 != 0 {
                let fp: Option<String> = conn
                    .query_row(
                        "SELECT file_path FROM clipboard_format WHERE entry_id = ?1 AND format_type IN (?2, ?3) AND file_path IS NOT NULL LIMIT 1",
                        params![entry.id, format::CF_DIB, format::CF_DIBV5],
                        |row| row.get(0),
                    )
                    .ok();
                entry.image_path = fp;
            }
        }

        Ok(entries)
    }

    /// Check if a content_hash already exists (for dedup check before capture).
    pub fn exists_by_hash(&self, content_hash: &str) -> Result<bool> {
        let conn = self.db.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM clipboard_entry WHERE content_hash = ?1",
            params![content_hash],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Toggle favorite status.
    pub fn toggle_favorite(&self, id: i64) -> Result<bool> {
        let conn = self.db.conn.lock().unwrap();
        conn.execute(
            "UPDATE clipboard_entry SET is_favorite = CASE WHEN is_favorite = 0 THEN 1 ELSE 0 END WHERE id = ?1",
            params![id],
        )?;
        let fav: i32 = conn.query_row(
            "SELECT is_favorite FROM clipboard_entry WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        Ok(fav != 0)
    }

    /// Delete a single entry and its formats (CASCADE).
    pub fn delete_entry(&self, id: i64) -> Result<()> {
        let conn = self.db.conn.lock().unwrap();
        conn.execute("DELETE FROM clipboard_entry WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Delete all entries (optionally keeping favorites).
    pub fn delete_all(&self, keep_favorites: bool) -> Result<Vec<String>> {
        let conn = self.db.conn.lock().unwrap();

        // Collect image paths for cleanup.
        let mut image_paths: Vec<String> = Vec::new();
        {
            let mut stmt = conn.prepare(
                "SELECT f.file_path FROM clipboard_format f
                 INNER JOIN clipboard_entry e ON e.id = f.entry_id
                 WHERE f.file_path IS NOT NULL",
            )?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            for row in rows {
                if let Ok(p) = row {
                    image_paths.push(p);
                }
            }
        }

        if keep_favorites {
            conn.execute("DELETE FROM clipboard_format WHERE entry_id IN (SELECT id FROM clipboard_entry WHERE is_favorite = 0)", params![])?;
            conn.execute("DELETE FROM clipboard_entry WHERE is_favorite = 0", params![])?;
        } else {
            conn.execute("DELETE FROM clipboard_entry", params![])?;
        }
        Ok(image_paths)
    }

    /// Cleanup entries older than retain_days (favorites exempted).
    pub fn cleanup(&self, retain_days: u32) -> Result<Vec<String>> {
        let conn = self.db.conn.lock().unwrap();

        let mut image_paths: Vec<String> = Vec::new();
        {
            let mut stmt = conn.prepare(
                "SELECT f.file_path FROM clipboard_format f
                 INNER JOIN clipboard_entry e ON e.id = f.entry_id
                 WHERE e.updated_at < datetime('now', '-' || ?1 || ' days')
                 AND e.is_favorite = 0
                 AND f.file_path IS NOT NULL",
            )?;
            let rows = stmt.query_map(params![retain_days], |row| row.get::<_, String>(0))?;
            for row in rows {
                if let Ok(p) = row {
                    image_paths.push(p);
                }
            }
        }

        conn.execute(
            "DELETE FROM clipboard_entry WHERE updated_at < datetime('now', '-' || ?1 || ' days') AND is_favorite = 0",
            params![retain_days],
        )?;

        Ok(image_paths)
    }

    /// Get statistics.
    pub fn get_stats(&self) -> Result<(i64, i64)> {
        let conn = self.db.conn.lock().unwrap();
        let (count, total_bytes): (i64, i64) = conn
            .query_row(
                "SELECT COUNT(*), COALESCE(SUM(content_length), 0) FROM clipboard_entry",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
        Ok((count, total_bytes))
    }

    /// Get format content for a specific format type.
    pub fn get_format_content(&self, entry_id: i64, format_type: i32) -> Result<Option<String>> {
        let conn = self.db.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT content FROM clipboard_format WHERE entry_id = ?1 AND format_type = ?2",
            params![entry_id, format_type],
            |row| row.get::<_, Option<String>>(0),
        )?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db;

    use std::sync::Mutex;

    fn setup_repo() -> Repository {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        db::migrate(&conn).unwrap();
        let dbc = DbConnection { conn: Mutex::new(conn) };
        Repository::new(dbc)
    }

    #[test]
    fn test_upsert_and_get() {
        let repo = setup_repo();
        let hash = "test_hash_1";
        let id = repo.upsert_entry(hash, "notepad.exe", "Untitled", 1, 5).unwrap();
        assert!(id > 0);

        // Dedup should return same id
        let id2 = repo.upsert_entry(hash, "notepad.exe", "Untitled", 1, 5).unwrap();
        assert_eq!(id, id2);
    }

    #[test]
    fn test_upsert_format() {
        let repo = setup_repo();
        let id = repo.upsert_entry("h1", "exe", "title", 1, 5).unwrap();
        repo.upsert_format(id, format::CF_UNICODETEXT, Some("hello"), None, "fh1")
            .unwrap();

        let content = repo.get_format_content(id, format::CF_UNICODETEXT).unwrap();
        assert_eq!(content, Some("hello".into()));
    }

    #[test]
    fn test_toggle_favorite() {
        let repo = setup_repo();
        let id = repo.upsert_entry("h1", "exe", "title", 1, 5).unwrap();
        // First toggle: 0→1, should return true (now favorite)
        assert!(repo.toggle_favorite(id).unwrap());
        // Second toggle: 1→0, should return false (now not favorite)
        assert!(!repo.toggle_favorite(id).unwrap());
    }

    #[test]
    fn test_delete_entry() {
        let repo = setup_repo();
        let id = repo.upsert_entry("h1", "exe", "title", 1, 5).unwrap();
        repo.delete_entry(id).unwrap();
        assert!(!repo.exists_by_hash("h1").unwrap());
    }

    #[test]
    fn test_get_stats() {
        let repo = setup_repo();
        repo.upsert_entry("h1", "a", "b", 1, 100).unwrap();
        repo.upsert_entry("h2", "a", "b", 1, 200).unwrap();
        let (count, bytes) = repo.get_stats().unwrap();
        assert_eq!(count, 2);
        assert_eq!(bytes, 300);
    }

    #[test]
    fn test_exists_by_hash() {
        let repo = setup_repo();
        assert!(!repo.exists_by_hash("nonexistent").unwrap());
        repo.upsert_entry("exists", "e", "t", 1, 0).unwrap();
        assert!(repo.exists_by_hash("exists").unwrap());
    }

    #[test]
    fn test_cursor_pagination() {
        let repo = setup_repo();
        for i in 0..5 {
            let hash = format!("h{i}");
            repo.upsert_entry(&hash, "exe", "title", 1, i).unwrap();
        }
        let entries = repo
            .get_history(0, "", 0, None, "updated_at", "desc", 3)
            .unwrap();
        assert_eq!(entries.len(), 3);
    }
}
