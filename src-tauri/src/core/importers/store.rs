//! Persists discovered games: upsert by `(source, sourceId)`.

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, params};

use crate::core::db::repo::{self, normalize_path};
use crate::core::types::{DiscoveredGame, ImportResult, ImportSource, MatchMode};
use crate::error::AppResult;

/// Adds an executable rule unless the game already has it. True when a row was added.
fn ensure_executable(
    conn: &Connection,
    game_id: i32,
    path: &str,
    mode: MatchMode,
) -> AppResult<bool> {
    let path = normalize_path(path);
    if path.is_empty() {
        return Ok(false);
    }
    let added = conn.execute(
        "INSERT OR IGNORE INTO game_executables (game_id, exe_path, match_mode) VALUES (?1, ?2, ?3)",
        params![game_id, path, mode.as_str()],
    )?;
    Ok(added > 0)
}

/// Writes launcher playtime to `imported_playtime`. True when the stored minutes changed.
pub fn set_imported_playtime(
    conn: &Connection,
    game_id: i32,
    source: ImportSource,
    minutes: i64,
    now: i64,
) -> AppResult<bool> {
    let current: Option<i64> = conn
        .query_row(
            "SELECT minutes FROM imported_playtime WHERE game_id = ?1 AND source = ?2",
            params![game_id, source.as_str()],
            |r| r.get(0),
        )
        .optional()?;
    conn.execute(
        "INSERT INTO imported_playtime (game_id, source, minutes, fetched_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(game_id, source) DO UPDATE SET minutes = excluded.minutes, fetched_at = excluded.fetched_at",
        params![game_id, source.as_str(), minutes, now],
    )?;
    Ok(current != Some(minutes))
}

/// Upserts `games` (all of one `source`) in one transaction and stamps `import_sources`.
///
/// Existing games keep their user-edited `name`, `hidden` and `afk_detection_enabled`; only
/// `install_dir`, the cover (when unset) and executables are refreshed, and executables are
/// only ever added. Launcher playtime goes to `imported_playtime`, never to `sessions`.
/// `covers` maps `sourceId` to an absolute cover file path.
pub fn import_games(
    conn: &Connection,
    source: ImportSource,
    games: &[DiscoveredGame],
    covers: &HashMap<String, String>,
    errors: Vec<String>,
    now: i64,
) -> AppResult<ImportResult> {
    let tx = conn.unchecked_transaction()?;
    let (mut added, mut updated, mut skipped) = (0, 0, 0);

    for found in games {
        let game_source = found.library_source();
        let existing = tx
            .query_row(
                "SELECT id, install_dir, cover_path FROM games WHERE source = ?1 AND source_id = ?2",
                params![game_source.as_str(), found.source_id],
                |r| {
                    Ok((
                        r.get::<_, i32>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()?;
        let cover = covers.get(&found.source_id);

        let (game_id, mut changed, is_new) = match existing {
            None => {
                let game = repo::insert_game(
                    &tx,
                    &found.name,
                    game_source,
                    Some(&found.source_id),
                    found.install_dir.as_deref(),
                    now,
                )?;
                (game.id, false, true)
            }
            Some((id, install_dir, cover_path)) => {
                let mut changed = false;
                if let Some(dir) = &found.install_dir
                    && install_dir.as_ref() != Some(dir)
                {
                    // Drop the rule made for the old directory; user and exact rules stay.
                    if let Some(old) = &install_dir {
                        let old = normalize_path(old);
                        if old != normalize_path(dir) {
                            tx.execute(
                                "DELETE FROM game_executables
                                 WHERE game_id = ?1 AND exe_path = ?2 AND match_mode = 'install_dir_prefix'",
                                params![id, old],
                            )?;
                        }
                    }
                    tx.execute(
                        "UPDATE games SET install_dir = ?1 WHERE id = ?2",
                        params![dir, id],
                    )?;
                    changed = true;
                }
                if cover.is_some() && cover_path.is_none() {
                    changed = true;
                }
                (id, changed, false)
            }
        };
        if let Some(cover) = cover
            && (is_new || changed)
        {
            tx.execute(
                "UPDATE games SET cover_path = ?1 WHERE id = ?2 AND cover_path IS NULL",
                params![cover, game_id],
            )?;
        }

        if let Some(dir) = &found.install_dir {
            changed |= ensure_executable(&tx, game_id, dir, MatchMode::InstallDirPrefix)?;
        }
        for exe in &found.executables {
            changed |= ensure_executable(&tx, game_id, exe, MatchMode::ExactPath)?;
        }
        if let Some(minutes) = found.playtime_minutes {
            changed |= set_imported_playtime(&tx, game_id, source, minutes, now)?;
        }

        if is_new {
            added += 1;
        } else if changed {
            updated += 1;
        } else {
            skipped += 1;
        }
    }

    tx.execute(
        "INSERT INTO import_sources (source, last_scan_at) VALUES (?1, ?2)
         ON CONFLICT(source) DO UPDATE SET last_scan_at = excluded.last_scan_at",
        params![source.as_str(), now],
    )?;
    tx.commit()?;

    Ok(ImportResult {
        source,
        discovered: games.len() as i32,
        added,
        updated,
        skipped,
        errors,
    })
}
