//! Favorite + copy-count mutations on entries.

use crate::repository::Repository;
use rusqlite::{params, Result as SqlResult};

impl Repository {
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
        self.read_copy_count(id)
    }

    /// Read the current copy_count for an entry by id.
    fn read_copy_count(&self, id: i64) -> SqlResult<i64> {
        self.db.query_row(
            "SELECT copy_count FROM entries WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
    }
}
