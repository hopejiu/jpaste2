//! SQLite data access layer (single source of truth).
//!
//! The `Repository` struct owns the connection and all query methods.
//! Methods are split across submodules by domain:
//! - `entries`: CRUD + pagination/search queries
//! - `favorites`: favorite + copy-count mutations
//! - `maintenance`: cleanup, clear, stats

use rusqlite::{Connection, Result as SqlResult, Row};
use std::path::Path;

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
}

pub(crate) mod entries;
pub(crate) mod favorites;
pub(crate) mod maintenance;
pub(crate) mod tests;
