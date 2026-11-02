//! SQLite access: connection setup, schema migrations, backup and restore.

pub mod library;
pub mod migrations;
pub mod repo;
pub mod seed;
pub mod stats;

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use crate::error::{AppError, AppResult};

/// Opens (creating if needed) the database at `path`, enables WAL and foreign keys,
/// and applies all pending migrations.
pub fn open(path: &Path) -> AppResult<Connection> {
    let mut conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrations::run(&mut conn)?;
    Ok(conn)
}

fn err(msg: impl Into<String>) -> AppError {
    AppError::Other(msg.into())
}

/// `<path><suffix>`, for example `gamelapse.db` + `.bak`.
fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name: OsString = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Writes a consistent copy of the database to `dest` with `VACUUM INTO`, replacing any
/// existing file there (the save dialog has already asked about overwriting).
pub fn backup_to(conn: &Connection, dest: &Path) -> AppResult<()> {
    if dest.exists() {
        std::fs::remove_file(dest)?;
    }
    let dest = dest
        .to_str()
        .ok_or_else(|| err("backup path is not valid UTF-8"))?;
    conn.execute("VACUUM INTO ?1", [dest])?;
    Ok(())
}

const SQLITE_MAGIC: &[u8; 16] = b"SQLite format 3\0";

/// Checks that `path` is a SQLite database with a `games` table, written by this version of
/// the app or an older one.
pub fn validate_backup(path: &Path) -> AppResult<()> {
    use std::io::Read;
    let mut header = [0u8; 16];
    let mut file = std::fs::File::open(path)?;
    if file.read_exact(&mut header).is_err() || &header != SQLITE_MAGIC {
        return Err(err("that file is not a SQLite database"));
    }
    drop(file);
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| err(format!("cannot open the backup: {e}")))?;
    let has_games: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'games')",
            [],
            |r| r.get(0),
        )
        .map_err(|e| err(format!("that file is not a valid backup: {e}")))?;
    if !has_games {
        return Err(err("that file is not a Gamelapse backup"));
    }
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version > migrations::count() as i64 {
        return Err(err("that backup was made by a newer version of Gamelapse"));
    }
    Ok(())
}

/// Replaces the database file at `db_path` with a copy of `backup`.
///
/// The caller holds the only handle on `conn` (the app's mutex guard). The live connection
/// is closed, the current file moves to `<db_path>.bak` (replacing an older one), stale
/// `-wal` and `-shm` files are removed, `backup` is copied in and opened with migrations
/// applied. On failure the previous file is put back and reopened, and the error returned.
pub fn replace_database(conn: &mut Connection, db_path: &Path, backup: &Path) -> AppResult<()> {
    validate_backup(backup)?;

    let _ = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()));
    let placeholder = Connection::open_in_memory()?;
    let old = std::mem::replace(conn, placeholder);
    if let Err((old, e)) = old.close() {
        *conn = old;
        return Err(err(format!("cannot close the database: {e}")));
    }

    let bak = with_suffix(db_path, ".bak");
    let wal = with_suffix(db_path, "-wal");
    let shm = with_suffix(db_path, "-shm");
    let had_db = db_path.exists();

    let swap = || -> AppResult<Connection> {
        if had_db {
            std::fs::rename(db_path, &bak)?;
        }
        let _ = std::fs::remove_file(&wal);
        let _ = std::fs::remove_file(&shm);
        std::fs::copy(backup, db_path)?;
        open(db_path)
    };
    match swap() {
        Ok(new) => {
            *conn = new;
            Ok(())
        }
        Err(e) => {
            // Put the previous database back.
            let _ = std::fs::remove_file(db_path);
            let _ = std::fs::remove_file(&wal);
            let _ = std::fs::remove_file(&shm);
            if had_db {
                let _ = std::fs::rename(&bak, db_path);
            }
            if let Ok(old) = open(db_path) {
                *conn = old;
            }
            Err(e)
        }
    }
}
