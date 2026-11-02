//! Adds the games the user confirmed in the scan dialog. This and `apply_game_metadata`
//! are the only paths from a scan into the library.

use std::path::Path;
use std::sync::{Mutex, PoisonError};

use rusqlite::Connection;

use super::roots::clean_root_path;
use crate::core::db::repo::{self, normalize_path};
use crate::core::importers::covers::{Downloader, download_to};
use crate::core::metadata;
use crate::core::types::{GameSource, ImportResult, ImportSource, MatchMode, ScanSelection};
use crate::error::AppResult;

fn lock(db: &Mutex<Connection>) -> std::sync::MutexGuard<'_, Connection> {
    db.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Checks that the executable is a file inside the folder, returning the cleaned folder.
fn validate(selection: &ScanSelection) -> Result<String, String> {
    if selection.name.trim().is_empty() {
        return Err(format!("{}: the name is empty", selection.folder));
    }
    let folder = clean_root_path(&selection.folder);
    if !Path::new(&folder).is_dir() {
        return Err(format!("{folder}: not a folder"));
    }
    let prefix = format!("{}\\", normalize_path(&folder).trim_end_matches('\\'));
    if !normalize_path(&selection.exe_path).starts_with(&prefix) {
        return Err(format!(
            "{}: the executable is not inside the folder",
            selection.name.trim()
        ));
    }
    if !Path::new(&selection.exe_path).is_file() {
        return Err(format!(
            "{}: the executable does not exist",
            selection.exe_path
        ));
    }
    Ok(folder)
}

/// Creates a `manual` game per selection with an `install_dir_prefix` rule for the folder
/// and an `exact_path` rule for the executable, then downloads its cover when one is given.
/// A selection whose executable already belongs to a game is skipped; a bad selection is
/// reported in `errors` and does not stop the others. The database lock is not held while
/// downloading.
///
/// The result's `source` is [`ImportSource::Heroic`] only because the type needs one; these
/// games are `manual`.
pub fn add_scanned_games(
    db: &Mutex<Connection>,
    data_dir: &Path,
    selections: &[ScanSelection],
    download: &Downloader,
    now: i64,
) -> AppResult<ImportResult> {
    let (mut added, mut skipped) = (0, 0);
    let mut errors = Vec::new();

    for selection in selections {
        let folder = match validate(selection) {
            Ok(f) => f,
            Err(e) => {
                errors.push(e);
                continue;
            }
        };
        let name = selection.name.trim();

        let game_id = {
            let conn = lock(db);
            if repo::executable_attached(&conn, &selection.exe_path)? {
                skipped += 1;
                continue;
            }
            let tx = conn.unchecked_transaction()?;
            let game = repo::insert_game(&tx, name, GameSource::Manual, None, Some(&folder), now)?;
            repo::insert_executable(&tx, game.id, &folder, MatchMode::InstallDirPrefix)?;
            repo::insert_executable(&tx, game.id, &selection.exe_path, MatchMode::ExactPath)?;
            tx.commit()?;
            game.id
        };
        added += 1;

        if let Some(url) = selection
            .cover_url
            .as_deref()
            .filter(|u| metadata::is_cover_url(u))
        {
            let path = metadata::lookup_cover_path(data_dir, game_id);
            match download_to(&path, url, download) {
                Ok(()) => {
                    let conn = lock(db);
                    repo::set_cover_path(&conn, game_id, Some(&path.to_string_lossy()))?;
                }
                Err(e) => tracing::debug!(game_id, error = %e, "cover download failed"),
            }
        }
    }

    Ok(ImportResult {
        source: ImportSource::Heroic,
        discovered: selections.len() as i32,
        added,
        updated: 0,
        skipped,
        errors,
    })
}
