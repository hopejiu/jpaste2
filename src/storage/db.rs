use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

/// Thread-safe wrapper around SQLite connection.
pub struct DbConnection {
    pub conn: Mutex<Connection>,
}

/// Open (or create) the SQLite database and run migrations.
pub fn init_db(db_path: &Path) -> Result<DbConnection> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;")?;
    migrate(&conn)?;

    Ok(DbConnection { conn: Mutex::new(conn) })
}

pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS clipboard_entry (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            content_hash    TEXT NOT NULL UNIQUE,
            source_exe      TEXT NOT NULL DEFAULT '',
            source_title    TEXT NOT NULL DEFAULT '',
            tag_mask        INTEGER NOT NULL DEFAULT 0,
            is_favorite     INTEGER NOT NULL DEFAULT 0,
            content_length  INTEGER NOT NULL DEFAULT 0,
            created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f', 'now')),
            updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%f', 'now'))
        );
        CREATE INDEX IF NOT EXISTS idx_entry_updated_at
            ON clipboard_entry(updated_at DESC, id DESC);

        CREATE TABLE IF NOT EXISTS clipboard_format (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            entry_id    INTEGER NOT NULL REFERENCES clipboard_entry(id) ON DELETE CASCADE,
            format_type INTEGER NOT NULL,
            content     TEXT,
            file_path   TEXT,
            format_hash TEXT NOT NULL,
            UNIQUE(entry_id, format_type)
        );
        CREATE INDEX IF NOT EXISTS idx_format_entry ON clipboard_format(entry_id);

        PRAGMA foreign_keys = ON;
        ",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_db_in_memory() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();

        // Verify tables exist.
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('clipboard_entry', 'clipboard_format')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_foreign_keys_enabled() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let enabled: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(enabled, 1);
    }
}
