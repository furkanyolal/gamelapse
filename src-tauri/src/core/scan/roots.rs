//! Saved scan roots: the folders the user chose to scan for games.

use std::path::Path;

use rusqlite::{Connection, params};

use crate::core::db::repo::normalize_path;
use crate::core::types::ScanRoot;
use crate::error::{AppError, AppResult};

fn err(msg: impl Into<String>) -> AppError {
    AppError::Other(msg.into())
}

/// Backslashes, no trailing separator (except a drive root), no `\\?\` prefix.
pub fn clean_root_path(path: &str) -> String {
    let mut p = path.trim().replace('/', "\\");
    if let Some(rest) = p.strip_prefix("\\\\?\\") {
        p = rest.to_string();
    }
    while p.len() > 3 && p.ends_with('\\') {
        p.pop();
    }
    p
}

/// `path` followed by exactly one separator, for prefix tests on normalised paths.
fn with_separator(normalized: &str) -> String {
    if normalized.ends_with('\\') {
        normalized.to_string()
    } else {
        format!("{normalized}\\")
    }
}

pub fn list(conn: &Connection) -> AppResult<Vec<ScanRoot>> {
    let mut stmt = conn
        .prepare("SELECT id, path, created_at FROM scan_roots ORDER BY path COLLATE NOCASE, id")?;
    let rows = stmt.query_map([], |r| {
        Ok(ScanRoot {
            id: r.get(0)?,
            path: r.get(1)?,
            created_at: r.get(2)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Adds a root. Fails when `path` is not a directory, is already a root, or lies inside or
/// contains an existing root.
pub fn add(conn: &Connection, path: &str, now: i64) -> AppResult<ScanRoot> {
    let clean = clean_root_path(path);
    if clean.is_empty() || !Path::new(&clean).is_dir() {
        return Err(err("that path is not a folder"));
    }
    let new = normalize_path(&clean);
    for existing in list(conn)? {
        let old = normalize_path(&existing.path);
        if old == new {
            return Err(err("that folder is already in the list"));
        }
        if new.starts_with(&with_separator(&old)) {
            return Err(err(format!(
                "that folder is inside {}, which is already in the list",
                existing.path
            )));
        }
        if old.starts_with(&with_separator(&new)) {
            return Err(err(format!(
                "that folder contains {}, which is already in the list",
                existing.path
            )));
        }
    }
    conn.execute(
        "INSERT INTO scan_roots (path, created_at) VALUES (?1, ?2)",
        params![clean, now],
    )?;
    Ok(ScanRoot {
        id: conn.last_insert_rowid() as i32,
        path: clean,
        created_at: now,
    })
}

/// Removes a root. Games that were added from it stay.
pub fn remove(conn: &Connection, id: i32) -> AppResult<()> {
    conn.execute("DELETE FROM scan_roots WHERE id = ?1", [id])?;
    Ok(())
}
