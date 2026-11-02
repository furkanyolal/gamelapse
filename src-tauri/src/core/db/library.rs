//! Library management: tags and merging duplicate games.
//! Every function takes a `&Connection`; callers decide about locking.

use std::collections::HashMap;

use rusqlite::{Connection, params};

use crate::core::db::repo;
use crate::core::types::{MergeResult, Tag};
use crate::error::{AppError, AppResult};

fn err(msg: impl Into<String>) -> AppError {
    AppError::Other(msg.into())
}

const MAX_TAG_LEN: usize = 40;

// ----------------------------------------------------------------- tags

/// Trimmed name, 1 to 40 characters.
fn validate_tag_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    let len = name.chars().count();
    if len == 0 {
        return Err(err("tag name must not be empty"));
    }
    if len > MAX_TAG_LEN {
        return Err(err(format!(
            "tag name must be at most {MAX_TAG_LEN} characters"
        )));
    }
    Ok(name.to_string())
}

fn map_tag(row: &rusqlite::Row) -> rusqlite::Result<Tag> {
    Ok(Tag {
        id: row.get(0)?,
        name: row.get(1)?,
    })
}

pub fn list_tags(conn: &Connection) -> AppResult<Vec<Tag>> {
    let mut stmt = conn.prepare("SELECT id, name FROM tags ORDER BY name COLLATE NOCASE, id")?;
    let rows = stmt.query_map([], map_tag)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn get_tag(conn: &Connection, id: i32) -> AppResult<Tag> {
    list_tags(conn)?
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| err(format!("tag {id} not found")))
}

/// Errors if another tag (not `except`) already has this name, ignoring case.
fn ensure_unique(conn: &Connection, name: &str, except: Option<i32>) -> AppResult<()> {
    let lower = name.to_lowercase();
    let taken = list_tags(conn)?
        .iter()
        .any(|t| Some(t.id) != except && t.name.to_lowercase() == lower);
    if taken {
        return Err(err(format!("a tag named \"{name}\" already exists")));
    }
    Ok(())
}

pub fn create_tag(conn: &Connection, name: &str) -> AppResult<Tag> {
    let name = validate_tag_name(name)?;
    ensure_unique(conn, &name, None)?;
    conn.execute("INSERT INTO tags (name) VALUES (?1)", [&name])?;
    Ok(Tag {
        id: conn.last_insert_rowid() as i32,
        name,
    })
}

pub fn rename_tag(conn: &Connection, id: i32, name: &str) -> AppResult<Tag> {
    let name = validate_tag_name(name)?;
    get_tag(conn, id)?;
    ensure_unique(conn, &name, Some(id))?;
    conn.execute("UPDATE tags SET name = ?1 WHERE id = ?2", params![name, id])?;
    Ok(Tag { id, name })
}

pub fn delete_tag(conn: &Connection, id: i32) -> AppResult<()> {
    get_tag(conn, id)?;
    conn.execute("DELETE FROM tags WHERE id = ?1", [id])?;
    Ok(())
}

/// The game's tags, sorted by name.
pub fn tags_for_game(conn: &Connection, game_id: i32) -> AppResult<Vec<Tag>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name FROM tags t JOIN game_tags gt ON gt.tag_id = t.id
         WHERE gt.game_id = ?1 ORDER BY t.name COLLATE NOCASE, t.id",
    )?;
    let rows = stmt.query_map([game_id], map_tag)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Every game's tags in one query, each list sorted by name.
pub fn tags_by_game(conn: &Connection) -> AppResult<HashMap<i32, Vec<Tag>>> {
    let mut stmt = conn.prepare(
        "SELECT gt.game_id, t.id, t.name FROM game_tags gt JOIN tags t ON t.id = gt.tag_id
         ORDER BY t.name COLLATE NOCASE, t.id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i32>(0)?,
            Tag {
                id: r.get(1)?,
                name: r.get(2)?,
            },
        ))
    })?;
    let mut map: HashMap<i32, Vec<Tag>> = HashMap::new();
    for row in rows {
        let (game_id, tag) = row?;
        map.entry(game_id).or_default().push(tag);
    }
    Ok(map)
}

/// Replaces the game's tags with `tag_ids` and returns them sorted by name.
pub fn set_game_tags(conn: &Connection, game_id: i32, tag_ids: &[i32]) -> AppResult<Vec<Tag>> {
    repo::get_game(conn, game_id)?;
    let known = list_tags(conn)?;
    for id in tag_ids {
        if !known.iter().any(|t| t.id == *id) {
            return Err(err(format!("tag {id} not found")));
        }
    }
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM game_tags WHERE game_id = ?1", [game_id])?;
    for id in tag_ids {
        tx.execute(
            "INSERT OR IGNORE INTO game_tags (game_id, tag_id) VALUES (?1, ?2)",
            params![game_id, id],
        )?;
    }
    tx.commit()?;
    tags_for_game(conn, game_id)
}

// ---------------------------------------------------------------- merge

/// Moves everything of `merged_ids` onto `survivor_id` and deletes the merged games, in one
/// transaction.
///
/// Sessions move as they are. Executables move unless the survivor already has the same path
/// (the table is unique on game and path, so a rule with a different mode counts as a
/// duplicate too). Tags are unioned. Imported playtime keeps the larger value per source.
/// Limits of merged games are deleted with them. Merged launcher games get an
/// `ignored_imports` tombstone, like a delete.
///
/// Errors, changing nothing, if the survivor is among the merged games, if any game does not
/// exist, or if any involved game has an active session.
pub fn merge_games(
    conn: &Connection,
    survivor_id: i32,
    merged_ids: &[i32],
    now: i64,
) -> AppResult<MergeResult> {
    let mut merged: Vec<i32> = Vec::new();
    for id in merged_ids {
        if !merged.contains(id) {
            merged.push(*id);
        }
    }
    if merged.is_empty() {
        return Err(err("choose at least one game to merge"));
    }
    if merged.contains(&survivor_id) {
        return Err(err("a game cannot be merged into itself"));
    }

    let tx = conn.unchecked_transaction()?;
    let involved = std::iter::once(survivor_id).chain(merged.iter().copied());
    for id in involved {
        let game = repo::get_game(&tx, id)?;
        if repo::active_session_for_game(&tx, id)?.is_some() {
            return Err(err(format!(
                "{} has an active session; stop it first",
                game.name
            )));
        }
    }

    let mut sessions = 0;
    let mut executables = 0;
    for id in &merged {
        sessions += tx.execute(
            "UPDATE sessions SET game_id = ?1 WHERE game_id = ?2",
            params![survivor_id, id],
        )?;
        executables += tx.execute(
            "UPDATE OR IGNORE game_executables SET game_id = ?1 WHERE game_id = ?2",
            params![survivor_id, id],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO game_tags (game_id, tag_id)
             SELECT ?1, tag_id FROM game_tags WHERE game_id = ?2",
            params![survivor_id, id],
        )?;
        tx.execute(
            "INSERT INTO imported_playtime (game_id, source, minutes, fetched_at)
             SELECT ?1, source, minutes, fetched_at FROM imported_playtime WHERE game_id = ?2
             ON CONFLICT(game_id, source) DO UPDATE SET
               fetched_at = CASE WHEN excluded.minutes > minutes
                                 THEN excluded.fetched_at ELSE fetched_at END,
               minutes = MAX(minutes, excluded.minutes)",
            params![survivor_id, id],
        )?;
    }
    for id in &merged {
        // Tombstone first so the next import does not recreate the game as a duplicate.
        repo::ignore_import(&tx, &repo::get_game(&tx, *id)?, now)?;
        // Cascades to leftover duplicate executables, tags, playtime rows and limits.
        tx.execute("DELETE FROM games WHERE id = ?1", [id])?;
    }
    tx.commit()?;
    Ok(MergeResult {
        survivor_id,
        merged_sessions: sessions as i32,
        merged_executables: executables as i32,
    })
}
