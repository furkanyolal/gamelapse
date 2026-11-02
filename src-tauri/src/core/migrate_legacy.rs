//! One-time copy of data from the pre-rename app folder (`GameTimeTracker`).

use std::io;
use std::path::Path;

/// Identifier of the app before it was renamed to Gamelapse.
pub const LEGACY_IDENTIFIER: &str = "com.furkanyolal.gametimetracker";
const LEGACY_DB: &str = "gametimetracker.db";
const NEW_DB: &str = "gamelapse.db";

/// Copies the old database, `settings.json` and `covers` folder from `legacy_dir`
/// into `new_dir`. Returns `true` when a copy happened. Does nothing when the
/// new database already exists or the old one is missing. Never touches `legacy_dir`.
pub fn migrate_legacy_data(legacy_dir: &Path, new_dir: &Path) -> io::Result<bool> {
    let legacy_db = legacy_dir.join(LEGACY_DB);
    if new_dir.join(NEW_DB).exists() || !legacy_db.is_file() {
        return Ok(false);
    }
    std::fs::create_dir_all(new_dir)?;

    let settings = legacy_dir.join("settings.json");
    let new_settings = new_dir.join("settings.json");
    if settings.is_file() && !new_settings.exists() {
        std::fs::copy(&settings, &new_settings)?;
    }
    let covers = legacy_dir.join("covers");
    if covers.is_dir() {
        copy_dir(&covers, &new_dir.join("covers"))?;
    }
    // The database goes last so an interrupted copy is retried on the next start. It is copied
    // to a temp name, fixed up, then renamed, so a failed fix-up never leaves a half-migrated db.
    let tmp_db = new_dir.join(format!("{NEW_DB}.migrating"));
    std::fs::copy(&legacy_db, &tmp_db)?;
    if let Err(e) = rewrite_cover_paths(&tmp_db, legacy_dir, new_dir) {
        let _ = std::fs::remove_file(&tmp_db);
        return Err(io::Error::other(format!("rewriting cover paths: {e}")));
    }
    std::fs::rename(&tmp_db, new_dir.join(NEW_DB))?;
    Ok(true)
}

/// Points `games.cover_path` values under `legacy_dir` at `new_dir` (case-insensitive prefix
/// match, as Windows paths are).
fn rewrite_cover_paths(db: &Path, legacy_dir: &Path, new_dir: &Path) -> rusqlite::Result<()> {
    let old = legacy_dir.to_string_lossy().into_owned();
    let new = new_dir.to_string_lossy().into_owned();
    let conn = rusqlite::Connection::open(db)?;
    conn.execute(
        "UPDATE games SET cover_path = ?1 || substr(cover_path, length(?2) + 1) \
         WHERE cover_path IS NOT NULL AND lower(substr(cover_path, 1, length(?2))) = lower(?2)",
        rusqlite::params![new, old],
    )?;
    // Fold the WAL into the main file before it is renamed.
    conn.pragma_update(None, "journal_mode", "DELETE")?;
    Ok(())
}

fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else if !target.exists() {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
