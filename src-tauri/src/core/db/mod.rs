//! SQLite access: connection setup and schema migrations.

pub mod migrations;
pub mod repo;
pub mod seed;
pub mod stats;

use std::path::Path;

use rusqlite::Connection;

use crate::error::AppResult;

/// Opens (creating if needed) the database at `path`, enables WAL and foreign keys,
/// and applies all pending migrations.
pub fn open(path: &Path) -> AppResult<Connection> {
    let mut conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrations::run(&mut conn)?;
    Ok(conn)
}
