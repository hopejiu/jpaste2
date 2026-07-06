use rusqlite::{params, Connection, Result as SqlResult, Row};
use std::path::Path;

use crate::model;

/// Whitelist of allowed sort fields for ORDER BY (SQL injection prevention).
const ALLOWED_SORT_FIELDS: &[&str] = &["updated_at", "content_length", "copy_count"];
/// Whitelist of allowed sort orders.
const ALLOWED_SORT_ORDERS: &[&str] = &["ASC", "DESC", "asc", "desc"];

/// Validate and return (sort_field, sort_order) against whitelists.
/// Falls back to defaults if invalid values are passed.
fn validate_sort_params<'a>(field: &'a str, order: &'a str) -> (&'a str, &'a str) {
    let f = if ALLOWED_SORT_FIELDS.contains(&field) {
        field
    } else {
        "updated_at"
    };
    let o = if ALLOWED_SORT_ORDERS.contains(&order) {
        order
    } else {
        "DESC"
    };
    (f, o)
}

/// Single source of truth for all SQLite data access.
pub struct Repository {
    #[cfg(test)]
    pub(crate) db: Connection,
    #[cfg(not(test))]
    db: Connection,
}

#[derive(Clone)]
pub struct EntryRow {
    pub id: i64,
    pub content_hash: String,
    pub content: String,
    pub image_path: String,
    pub thumb_path: String,
    pub tag_mask: i32,
    pub is_favorite: bool,
    pub content_length: i64,
    pub copy_count: i64,
    pub qr_text: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Map a rusqlite Row to EntryRow.
impl TryFrom<&Row<'_>> for EntryRow {
    type Error = rusqlite::Error;
    fn try_from(row: &Row<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.get(0)?,
            content_hash: row.get(1)?,
            content: row.get(2)?,
            image_path: row.get(3)?,
            thumb_path: row.get(4)?,
            tag_mask: row.get(5)?,
            is_favorite: row.get::<_, i32>(6)? != 0,
            content_length: row.get(7)?,
            copy_count: row.get(8)?,
            qr_text: row.get(9)?,
            created_at: row.get(10)?,
            updated_at: row.get(11)?,
        })
    }
}

/// The 12-column SELECT list that matches the EntryRow column order.
const ENTRY_SELECT_COLS: &str = "id, content_hash, content, image_path, thumb_path, tag_mask, is_favorite, content_length, copy_count, qr_text, created_at, updated_at";

impl Repository {
    /// Open or create the SQLite database at `db_path` and run migrations.
    pub fn new(db_path: &Path) -> SqlResult<Self> {
        log::info!("repository: opening database at {:?}", db_path);
        let db = Connection::open(db_path)?;
        let repo = Self { db };
        repo.configure()?;
        repo.migrate()?;
        log::info!("repository: database initialized successfully");
        Ok(repo)
    }

    /// Configure SQLite pragmas for performance.
    /// Called once before migration.
    fn configure(&self) -> SqlResult<()> {
        self.db.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -8000;
             PRAGMA mmap_size = 268435456;
             PRAGMA temp_store = MEMORY;",
        )
    }

    /// Run schema migrations
    pub fn migrate(&self) -> SqlResult<()> {
        self.db.execute_batch(
            "CREATE TABLE IF NOT EXISTS entries (
                id             INTEGER PRIMARY KEY AUTOINCREMENT,
                content_hash   TEXT NOT NULL UNIQUE,
                content        TEXT NOT NULL DEFAULT '',
                image_path     TEXT NOT NULL DEFAULT '',
                thumb_path     TEXT NOT NULL DEFAULT '',
                tag_mask       INTEGER NOT NULL DEFAULT 0,
                is_favorite    INTEGER NOT NULL DEFAULT 0,
                content_length INTEGER NOT NULL DEFAULT 0,
                copy_count     INTEGER NOT NULL DEFAULT 0,
                no_auto_fav    INTEGER NOT NULL DEFAULT 0,
                created_at     INTEGER NOT NULL,
                updated_at     INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_entries_updated ON entries(updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_entries_hash ON entries(content_hash);
            CREATE INDEX IF NOT EXISTS idx_entries_favorite ON entries(is_favorite);
            CREATE INDEX IF NOT EXISTS idx_entries_image ON entries(image_path) WHERE image_path != '';"
        )?;
        // Add thumb_path column only for databases created before it existed.
        // Probe via PRAGMA so a fresh DB (where the column already exists from
        // the CREATE TABLE above) doesn't trigger a spurious "duplicate column"
        // error that was previously swallowed by `let _ =`. A genuine failure
        // (e.g. corrupt DB) now surfaces instead of panicking later at runtime.
        let has_thumb: i64 = self
            .db
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('entries') WHERE name = 'thumb_path'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if has_thumb == 0 {
            self.db.execute_batch(
                "ALTER TABLE entries ADD COLUMN thumb_path TEXT NOT NULL DEFAULT '';",
            )?;
        }
        // Add copy_count column for databases created before it existed.
        let has_copy_count: i64 = self
            .db
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('entries') WHERE name = 'copy_count'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if has_copy_count == 0 {
            self.db.execute_batch(
                "ALTER TABLE entries ADD COLUMN copy_count INTEGER NOT NULL DEFAULT 0;",
            )?;
        }
        // Add no_auto_fav column for databases created before it existed.
        let has_no_auto_fav: i64 = self
            .db
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('entries') WHERE name = 'no_auto_fav'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if has_no_auto_fav == 0 {
            self.db.execute_batch(
                "ALTER TABLE entries ADD COLUMN no_auto_fav INTEGER NOT NULL DEFAULT 0;",
            )?;
        }
        // Add qr_text column for databases created before it existed.
        let has_qr_text: i64 = self
            .db
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('entries') WHERE name = 'qr_text'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if has_qr_text == 0 {
            self.db.execute_batch(
                "ALTER TABLE entries ADD COLUMN qr_text TEXT NOT NULL DEFAULT '';",
            )?;
        }
        Ok(())
    }

    /// Generate current timestamp in Unix milliseconds
    pub fn now() -> i64 {
        crate::util::chrono_now()
    }

    /// Upsert by content_hash: if exists, update updated_at, tag_mask, content, content_length,
    /// increment copy_count, and return true (was dedup). Also returns the new copy_count.
    pub fn upsert_dedup(
        &self,
        hash: &str,
        tag_mask: i32,
        content: &str,
        content_length: i64,
    ) -> SqlResult<(bool, i64)> {
        let now = Self::now();
        let rows = self.db.execute(
            "UPDATE entries SET updated_at = ?1, tag_mask = ?2, content = ?3, content_length = ?4, copy_count = copy_count + 1
             WHERE content_hash = ?5",
            params![now, tag_mask, content, content_length, hash],
        )?;
        if rows > 0 {
            // Read back the new copy_count
            let cc: i64 = self.db.query_row(
                "SELECT copy_count FROM entries WHERE content_hash = ?1",
                params![hash],
                |row| row.get(0),
            )?;
            Ok((true, cc))
        } else {
            Ok((false, 0))
        }
    }

    /// Insert a new entry, returning its ID
    pub fn insert_entry(
        &self,
        hash: &str,
        content: &str,
        tag_mask: i32,
        content_length: i64,
        qr_text: &str,
    ) -> SqlResult<i64> {
        let now = Self::now();
        self.db.execute(
            "INSERT INTO entries (content_hash, content, tag_mask, content_length, qr_text, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![hash, content, tag_mask, content_length, qr_text, now, now],
        )?;
        let id = self.db.last_insert_rowid();
        Ok(id)
    }

    /// Set qr_text for an entry
    pub fn set_qr_text(&self, id: i64, qr_text: &str) -> SqlResult<()> {
        self.db.execute(
            "UPDATE entries SET qr_text = ?1 WHERE id = ?2",
            params![qr_text, id],
        )?;
        Ok(())
    }

    /// Set image_path for an entry
    pub fn set_image_path(&self, id: i64, path: &str) -> SqlResult<()> {
        self.db.execute(
            "UPDATE entries SET image_path = ?1 WHERE id = ?2",
            params![path, id],
        )?;
        Ok(())
    }

    /// Set thumb_path for an entry
    pub fn set_thumb_path(&self, id: i64, path: &str) -> SqlResult<()> {
        self.db.execute(
            "UPDATE entries SET thumb_path = ?1 WHERE id = ?2",
            params![path, id],
        )?;
        Ok(())
    }

    /// Update timestamp only (for dedup refresh)
    pub fn update_timestamp(&self, id: i64) -> SqlResult<()> {
        let now = Self::now();
        self.db.execute(
            "UPDATE entries SET updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    /// Build a `tag_mask & ? != 0` WHERE clause (no favorite handling).
    /// Increments `param_idx` and appends the bound parameter.
    fn tag_clause(
        tag_mask: i32,
        param_idx: &mut u32,
        params: &mut Vec<rusqlite::types::Value>,
    ) -> Option<String> {
        if tag_mask == 0 {
            return None;
        }
        *param_idx += 1;
        params.push(tag_mask.into());
        Some(format!("tag_mask & ?{} != 0", param_idx))
    }

    /// Build a `content LIKE ?` WHERE clause.
    /// When `escape` is true, `%`/`_` are escaped and an `ESCAPE '\\'`
    /// qualifier is appended (used by entry queries). When false, the raw
    /// pattern is used (preserves `get_image_list` historical behavior).
    /// Increments `param_idx` and appends the bound parameter.
    fn search_clause(
        search: &str,
        param_idx: &mut u32,
        params: &mut Vec<rusqlite::types::Value>,
        escape: bool,
    ) -> Option<String> {
        if search.is_empty() {
            return None;
        }
        *param_idx += 1;
        let pattern = if escape {
            let escaped = search
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_");
            format!("%{}%", escaped)
        } else {
            format!("%{}%", search)
        };
        params.push(pattern.into());
        Some(if escape {
            format!("content LIKE ?{} ESCAPE '\\'", param_idx)
        } else {
            format!("content LIKE ?{}", param_idx)
        })
    }

    /// Build the WHERE clauses, ORDER BY, and params vector for entry queries.
    /// Returns (where_sql_string, order_by_string, param_values).
    /// The `select_cols` parameter allows the caller to choose what columns to SELECT.
    fn build_entry_query_parts(
        &self,
        search: &str,
        tag_mask: i32,
        after_updated: i64,
        after_id: i64,
        limit: i32,
        sort_field: &str,
        sort_order: &str,
    ) -> (String, String, String, Vec<rusqlite::types::Value>) {
        let (sf, so) = validate_sort_params(sort_field, sort_order);

        let effective_mask = if tag_mask & crate::model::TAG_FAVORITE != 0 {
            tag_mask & !crate::model::TAG_FAVORITE
        } else {
            tag_mask
        };

        let mut where_clauses: Vec<String> = Vec::new();
        if tag_mask & crate::model::TAG_FAVORITE != 0 {
            where_clauses.push("is_favorite = 1".into());
        }
        let mut params_vec: Vec<rusqlite::types::Value> = Vec::new();
        let mut param_idx = 0u32;
        if let Some(c) = Self::tag_clause(effective_mask, &mut param_idx, &mut params_vec) {
            where_clauses.push(c);
        }
        if let Some(c) = Self::search_clause(search, &mut param_idx, &mut params_vec, true) {
            where_clauses.push(c);
        }

        let where_sql = if where_clauses.is_empty() {
            String::new()
        } else {
            format!(" AND {}", where_clauses.join(" AND "))
        };

        let has_cursor = after_updated != 0;
        // ponytail: cursor field matches sort_field for consistent pagination.
        // Falls back to updated_at if sort_field doesn't need cursor (copy_count).
        let cursor_field = if sf == "copy_count" { "copy_count" } else { "updated_at" };
        let cursor_sql = if has_cursor {
            let op = if so == "DESC" { "<" } else { ">" };
            format!(
                " AND ({} {} ?{} OR ({} = ?{} AND id {} ?{}))",
                cursor_field,
                op,
                param_idx + 1,
                cursor_field,
                param_idx + 1,
                op,
                param_idx + 2
            )
        } else {
            String::new()
        };

        if has_cursor {
            params_vec.push(rusqlite::types::Value::from(after_updated));
            params_vec.push(rusqlite::types::Value::from(after_id));
            // LIMIT is at param_idx + 3
            params_vec.push(rusqlite::types::Value::from(limit));
        } else {
            params_vec.push(rusqlite::types::Value::from(limit));
        };

        let order_sql = format!(
            " ORDER BY {} {}, id {} LIMIT ?{}",
            sf, so, so,
            param_idx + if has_cursor { 3 } else { 1 }
        );

        (where_sql, cursor_sql, order_sql, params_vec)
    }

    /// Query entries with cursor-based pagination, tag filter, search, and sorting
    #[allow(clippy::too_many_arguments)]
    pub fn query_entries(
        &self,
        search: &str,
        tag_mask: i32,
        after_updated: i64,
        after_id: i64,
        limit: i32,
        sort_field: &str,
        sort_order: &str,
    ) -> SqlResult<Vec<EntryRow>> {
        let (where_sql, cursor_sql, order_sql, params_vec) = self.build_entry_query_parts(
            search,
            tag_mask,
            after_updated,
            after_id,
            limit,
            sort_field,
            sort_order,
        );

        let has_cursor = after_updated != 0;
        let full_sql = if has_cursor {
            format!(
                "SELECT {} FROM entries WHERE 1=1{}{}{}",
                ENTRY_SELECT_COLS, where_sql, cursor_sql, order_sql
            )
        } else {
            format!(
                "SELECT {} FROM entries WHERE 1=1{}{}",
                ENTRY_SELECT_COLS, where_sql, order_sql
            )
        };

        let mut stmt = self.db.prepare(&full_sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params_vec.iter()), |row| {
            EntryRow::try_from(row)
        })?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// Lightweight query for regex search: only fetches id + content + updated_at.
    /// Avoids cloning large EntryRow structs.
    pub fn query_entries_regex_light(
        &self,
        tag_mask: i32,
        after_updated: i64,
        after_id: i64,
        limit: i32,
        sort_field: &str,
        sort_order: &str,
    ) -> SqlResult<Vec<(i64, String, i64)>> {
        // Reuse shared query builder (empty search)
        let (where_sql, cursor_sql, order_sql, params_vec) = self.build_entry_query_parts(
            "",
            tag_mask,
            after_updated,
            after_id,
            limit,
            sort_field,
            sort_order,
        );

        let has_cursor = after_updated != 0;
        let full_sql = if has_cursor {
            format!(
                "SELECT id, content, updated_at FROM entries WHERE 1=1{}{}{}",
                where_sql, cursor_sql, order_sql
            )
        } else {
            format!(
                "SELECT id, content, updated_at FROM entries WHERE 1=1{}{}",
                where_sql, order_sql
            )
        };

        let mut stmt = self.db.prepare(&full_sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params_vec.iter()), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// Get single entry by ID
    pub fn get_entry(&self, id: i64) -> SqlResult<EntryRow> {
        self.db.query_row(
            &format!("SELECT {} FROM entries WHERE id = ?1", ENTRY_SELECT_COLS),
            params![id],
            |row| EntryRow::try_from(row),
        )
    }

    /// Delete entry by ID, returning (image_path, thumb_path) if any
    pub fn delete_entry(&self, id: i64) -> SqlResult<(Option<String>, Option<String>)> {
        let row = self
            .db
            .query_row(
                "SELECT image_path, thumb_path FROM entries WHERE id = ?1",
                params![id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .ok();

        self.db
            .execute("DELETE FROM entries WHERE id = ?1", params![id])?;

        match row {
            Some((img, thumb)) => {
                let img_opt = if img.is_empty() { None } else { Some(img) };
                let thumb_opt = if thumb.is_empty() { None } else { Some(thumb) };
                Ok((img_opt, thumb_opt))
            }
            None => Ok((None, None)),
        }
    }

    /// Toggle favorite status
    pub fn toggle_favorite(&self, id: i64, value: bool) -> SqlResult<()> {
        let v: i32 = if value { 1 } else { 0 };
        self.db.execute(
            "UPDATE entries SET is_favorite = ?1 WHERE id = ?2",
            params![v, id],
        )?;
        Ok(())
    }

    /// Set the no_auto_fav flag (prevents auto-favorite from triggering again).
    pub fn set_no_auto_fav(&self, id: i64, value: bool) -> SqlResult<()> {
        let v: i32 = if value { 1 } else { 0 };
        self.db.execute(
            "UPDATE entries SET no_auto_fav = ?1 WHERE id = ?2",
            params![v, id],
        )?;
        Ok(())
    }

    /// Check whether an entry has the no_auto_fav flag set.
    pub fn has_no_auto_fav_flag(&self, id: i64) -> SqlResult<bool> {
        self.db.query_row(
            "SELECT no_auto_fav FROM entries WHERE id = ?1",
            params![id],
            |row| Ok(row.get::<_, i32>(0)? != 0),
        )
    }

    /// Increment copy_count for an entry (used on user paste/copy from jPaste).
    /// Returns the new copy_count.
    pub fn increment_copy_count(&self, id: i64) -> SqlResult<i64> {
        self.db.execute(
            "UPDATE entries SET copy_count = copy_count + 1 WHERE id = ?1",
            params![id],
        )?;
        self.db.query_row(
            "SELECT copy_count FROM entries WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
    }

    /// Read the current copy_count for an entry.
    #[allow(dead_code)]
    pub fn get_copy_count(&self, id: i64) -> SqlResult<i64> {
        self.db.query_row(
            "SELECT copy_count FROM entries WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
    }

    /// Delete entries older than retain_days, respecting favorites
    /// Returns (deleted_count, list_of_file_paths_to_remove)
    /// Uses SQLite RETURNING clause (3.35+) to avoid separate SELECT.
    pub fn cleanup(&self, retain_days: u32) -> SqlResult<(u64, Vec<String>)> {
        // Use bound parameter instead of format! for cutoff
        let sql = "DELETE FROM entries WHERE updated_at < (CAST(strftime('%s', 'now') AS INTEGER) - CAST(?1 AS INTEGER) * 86400) * 1000 AND is_favorite = 0 RETURNING image_path, thumb_path";

        let mut stmt = self.db.prepare(sql)?;
        let rows = stmt.query_map(params![retain_days], |row| {
            let img: String = row.get(0)?;
            let thumb: String = row.get(1)?;
            Ok((img, thumb))
        })?;

        let mut paths = Vec::new();
        let mut count = 0u64;
        for row in rows {
            count += 1;
            if let Ok((img, thumb)) = row {
                if !img.is_empty() {
                    paths.push(img);
                }
                if !thumb.is_empty() {
                    paths.push(thumb);
                }
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
        let paths = Self::collect_image_paths(&self.db, where_clause)?;

        if keep_favorites {
            self.db
                .execute("DELETE FROM entries WHERE is_favorite = 0", [])?;
        } else {
            self.db.execute("DELETE FROM entries", [])?;
        };

        Ok(paths)
    }

    /// Collect image and thumb paths matching an optional WHERE clause.
    fn collect_image_paths(db: &Connection, where_clause: &str) -> SqlResult<Vec<String>> {
        let sql = format!("SELECT image_path, thumb_path FROM entries{}", where_clause);
        let mut stmt = db.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            let img: String = row.get(0)?;
            let thumb: String = row.get(1)?;
            Ok((img, thumb))
        })?;
        let mut result = Vec::new();
        for row in rows {
            if let Ok((img, thumb)) = row {
                if !img.is_empty() {
                    result.push(img);
                }
                if !thumb.is_empty() {
                    result.push(thumb);
                }
            }
        }
        Ok(result)
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

    /// Find entry ID by content_hash. Returns the ID or 0 if not found.
    /// Uses the hash index for O(log n) lookup.
    pub fn find_id_by_hash(&self, hash: &str) -> SqlResult<i64> {
        match self.db.query_row(
            "SELECT id FROM entries WHERE content_hash = ?1",
            params![hash],
            |row| row.get::<_, i64>(0),
        ) {
            Ok(id) => Ok(id),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(0),
            Err(e) => Err(e),
        }
    }

    /// Get total image file size on disk (uses partial index for efficiency).
    /// Collects paths first, then does file I/O outside the query iteration.
    pub fn get_image_storage_bytes(&self, app_data: &std::path::Path) -> SqlResult<i64> {
        let sql = "SELECT image_path, thumb_path FROM entries WHERE image_path != ''";
        let mut stmt = self.db.prepare(sql)?;
        let rows = stmt.query_map([], |row| {
            let img: String = row.get(0)?;
            let thumb: String = row.get(1)?;
            Ok((img, thumb))
        })?;

        // Collect paths first to minimize time spent in query iteration
        let paths: Vec<(String, String)> = rows.filter_map(|r| r.ok()).collect();

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

    /// Get image entry IDs matching the given tag mask and search,
    /// ordered by updated_at DESC. Used by the image viewer for prev/next navigation.
    pub fn get_image_list(&self, tag_mask: i32, search: &str) -> SqlResult<Vec<i64>> {
        let mut conditions: Vec<String> = Vec::new();
        let mut params_vec: Vec<rusqlite::types::Value> = Vec::new();
        let mut param_idx = 0u32;

        if let Some(c) = Self::tag_clause(tag_mask, &mut param_idx, &mut params_vec) {
            conditions.push(c);
        }
        if let Some(c) = Self::search_clause(search, &mut param_idx, &mut params_vec, false) {
            conditions.push(c);
        }

        let where_extra = if conditions.is_empty() {
            String::new()
        } else {
            format!(" AND {}", conditions.join(" AND "))
        };

        let sql = format!(
            "SELECT id FROM entries WHERE image_path != ''{} ORDER BY updated_at DESC",
            where_extra
        );

        let mut stmt = self.db.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params_vec.iter()), |row| {
            row.get::<_, i64>(0)
        })?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod repository_tests {
    use super::*;

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
    use super::*;
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
}
