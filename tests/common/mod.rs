//! Shared test utilities for integration tests.
//! Each test crate in `tests/` compiles independently, so `dead_code` is expected.

#![allow(dead_code)]

use std::path::Path;
use std::sync::Mutex;

use jpastev2::settings::SettingsService;
use jpastev2::storage::db::{self, DbConnection};
use jpastev2::Repository;

/// Create an in-memory Repository with migrated schema.
pub fn temp_repo() -> Repository {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    db::migrate(&conn).unwrap();
    Repository::new(DbConnection {
        conn: Mutex::new(conn),
    })
}

/// Create a SettingsService pointing at a temp directory.
pub fn temp_settings(dir: &Path) -> SettingsService {
    SettingsService::load(&dir.to_path_buf()).unwrap()
}

/// Insert a test entry with text content and return the entry ID.
pub fn insert_text_entry(repo: &Repository, text: &str, hash: &str) -> i64 {
    let eid = repo
        .upsert_entry(hash, "test.exe", "Test Window", 1, text.len() as i32)
        .unwrap();
    repo.upsert_format(eid, 13, Some(text), None, hash)
        .unwrap();
    eid
}
