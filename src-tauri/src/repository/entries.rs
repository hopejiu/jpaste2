//! Entry CRUD + pagination/search queries.

use crate::repository::{EntryRow, Repository};
use rusqlite::{params, Result as SqlResult};

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

/// The 12-column SELECT list that matches the EntryRow column order.
const ENTRY_SELECT_COLS: &str = "id, content_hash, content, image_path, thumb_path, tag_mask, is_favorite, content_length, copy_count, qr_text, created_at, updated_at";

impl Repository {
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
        // All ALLOWED_SORT_FIELDS are real columns, so reuse sf directly.
        let cursor_field = if matches!(sf, "updated_at" | "content_length" | "copy_count") {
            sf
        } else {
            "updated_at"
        };
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
