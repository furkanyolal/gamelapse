//! Repositories for games, executables, sessions and heartbeats.
//! Every function takes a `&Connection`; callers decide about locking.

use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::core::types::{
    Game, GameExecutable, GameSource, GameUpdate, MatchMode, Session, SessionQuery, SessionSource,
    SessionUpdate, SessionWithGame,
};
use crate::error::{AppError, AppResult};

fn err(msg: impl Into<String>) -> AppError {
    AppError::Other(msg.into())
}

/// Lowercase, backslashes, no trailing separator, no `\\?\` prefix.
pub fn normalize_path(path: &str) -> String {
    let mut p = path.trim().replace('/', "\\").to_lowercase();
    if let Some(rest) = p.strip_prefix("\\\\?\\") {
        p = rest.to_string();
    }
    while p.len() > 3 && p.ends_with('\\') {
        p.pop();
    }
    p
}

/// Lowercase, with a leading "the " dropped.
pub fn sort_name(name: &str) -> String {
    let lower = name.trim().to_lowercase();
    match lower.strip_prefix("the ") {
        Some(rest) if !rest.trim().is_empty() => rest.trim_start().to_string(),
        _ => lower,
    }
}

/// `max(0, (ended or now) - started - afk)`.
pub fn session_duration(started_at: i64, ended_at: Option<i64>, afk_secs: i64, now: i64) -> i64 {
    (ended_at.unwrap_or(now) - started_at - afk_secs).max(0)
}

// ---------------------------------------------------------------- games

const GAME_COLS: &str = "id, name, sort_name, source, source_id, install_dir, icon_path, cover_path, hidden, afk_detection_enabled, created_at";

fn map_game(row: &Row) -> rusqlite::Result<Game> {
    let source: String = row.get(3)?;
    Ok(Game {
        id: row.get(0)?,
        name: row.get(1)?,
        sort_name: row.get(2)?,
        source: GameSource::parse(&source).unwrap_or(GameSource::Manual),
        source_id: row.get(4)?,
        install_dir: row.get(5)?,
        icon_path: row.get(6)?,
        cover_path: row.get(7)?,
        hidden: row.get(8)?,
        afk_detection_enabled: row.get(9)?,
        created_at: row.get(10)?,
    })
}

pub fn get_game(conn: &Connection, id: i32) -> AppResult<Game> {
    conn.query_row(
        &format!("SELECT {GAME_COLS} FROM games WHERE id = ?1"),
        [id],
        map_game,
    )
    .optional()?
    .ok_or_else(|| err(format!("game {id} not found")))
}

pub fn list_games(conn: &Connection, include_hidden: bool) -> AppResult<Vec<Game>> {
    let sql =
        format!("SELECT {GAME_COLS} FROM games WHERE (?1 OR hidden = 0) ORDER BY sort_name, id");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([include_hidden], map_game)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn insert_game(
    conn: &Connection,
    name: &str,
    source: GameSource,
    source_id: Option<&str>,
    install_dir: Option<&str>,
    now: i64,
) -> AppResult<Game> {
    conn.execute(
        "INSERT INTO games (name, sort_name, source, source_id, install_dir, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            name,
            sort_name(name),
            source.as_str(),
            source_id,
            install_dir,
            now
        ],
    )?;
    get_game(conn, conn.last_insert_rowid() as i32)
}

pub fn update_game(conn: &Connection, id: i32, patch: &GameUpdate) -> AppResult<Game> {
    let game = get_game(conn, id)?;
    let name = match &patch.name {
        Some(n) => {
            let n = n.trim();
            if n.is_empty() {
                return Err(err("name must not be empty"));
            }
            n.to_string()
        }
        None => game.name.clone(),
    };
    conn.execute(
        "UPDATE games SET name = ?1, sort_name = ?2, hidden = ?3, afk_detection_enabled = ?4 WHERE id = ?5",
        params![
            name,
            sort_name(&name),
            patch.hidden.unwrap_or(game.hidden),
            patch.afk_detection_enabled.unwrap_or(game.afk_detection_enabled),
            id
        ],
    )?;
    get_game(conn, id)
}

pub fn delete_game(conn: &Connection, id: i32) -> AppResult<()> {
    get_game(conn, id)?;
    if active_session_for_game(conn, id)?.is_some() {
        return Err(err("game has an active session; stop it first"));
    }
    conn.execute("DELETE FROM games WHERE id = ?1", [id])?;
    Ok(())
}

/// The Steam game with this app id, if it is in the library.
pub fn steam_game_for_app(conn: &Connection, app_id: u32) -> AppResult<Option<i32>> {
    Ok(conn
        .query_row(
            "SELECT id FROM games WHERE source = 'steam' AND source_id = ?1",
            [app_id.to_string()],
            |r| r.get(0),
        )
        .optional()?)
}

// ---------------------------------------------------------- executables

fn map_exe(row: &Row) -> rusqlite::Result<GameExecutable> {
    let mode: String = row.get(3)?;
    Ok(GameExecutable {
        id: row.get(0)?,
        game_id: row.get(1)?,
        exe_path: row.get(2)?,
        match_mode: MatchMode::parse(&mode).unwrap_or(MatchMode::ExactPath),
    })
}

pub fn list_executables(conn: &Connection, game_id: i32) -> AppResult<Vec<GameExecutable>> {
    let mut stmt = conn.prepare(
        "SELECT id, game_id, exe_path, match_mode FROM game_executables WHERE game_id = ?1 ORDER BY id",
    )?;
    let rows = stmt.query_map([game_id], map_exe)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn list_all_executables(conn: &Connection) -> AppResult<Vec<GameExecutable>> {
    let mut stmt =
        conn.prepare("SELECT id, game_id, exe_path, match_mode FROM game_executables ORDER BY id")?;
    let rows = stmt.query_map([], map_exe)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn insert_executable(
    conn: &Connection,
    game_id: i32,
    exe_path: &str,
    mode: MatchMode,
) -> AppResult<GameExecutable> {
    get_game(conn, game_id)?;
    let path = normalize_path(exe_path);
    if path.is_empty() {
        return Err(err("executable path must not be empty"));
    }
    let inserted = conn.execute(
        "INSERT OR IGNORE INTO game_executables (game_id, exe_path, match_mode) VALUES (?1, ?2, ?3)",
        params![game_id, path, mode.as_str()],
    )?;
    if inserted == 0 {
        return Err(err("that executable is already attached to the game"));
    }
    let id = conn.last_insert_rowid() as i32;
    Ok(conn.query_row(
        "SELECT id, game_id, exe_path, match_mode FROM game_executables WHERE id = ?1",
        [id],
        map_exe,
    )?)
}

pub fn remove_executable(conn: &Connection, id: i32) -> AppResult<()> {
    conn.execute("DELETE FROM game_executables WHERE id = ?1", [id])?;
    Ok(())
}

/// True if any game already has this (normalised) path as an executable.
pub fn executable_attached(conn: &Connection, exe_path: &str) -> AppResult<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM game_executables WHERE exe_path = ?1 LIMIT 1",
            [normalize_path(exe_path)],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

// ------------------------------------------------------------- sessions

const SESSION_COLS: &str =
    "s.id, s.game_id, s.started_at, s.ended_at, s.afk_secs, s.source, s.note";

fn map_session(row: &Row, now: i64) -> rusqlite::Result<Session> {
    let started_at: i64 = row.get(2)?;
    let ended_at: Option<i64> = row.get(3)?;
    let afk_secs: i64 = row.get(4)?;
    let source: String = row.get(5)?;
    Ok(Session {
        id: row.get(0)?,
        game_id: row.get(1)?,
        started_at,
        ended_at,
        duration_secs: session_duration(started_at, ended_at, afk_secs, now),
        afk_secs,
        source: SessionSource::parse(&source).unwrap_or(SessionSource::Auto),
        note: row.get(6)?,
    })
}

pub fn get_session(conn: &Connection, id: i32, now: i64) -> AppResult<Session> {
    conn.query_row(
        &format!("SELECT {SESSION_COLS} FROM sessions s WHERE s.id = ?1"),
        [id],
        |r| map_session(r, now),
    )
    .optional()?
    .ok_or_else(|| err(format!("session {id} not found")))
}

pub fn insert_session(
    conn: &Connection,
    game_id: i32,
    started_at: i64,
    ended_at: Option<i64>,
    source: SessionSource,
    note: Option<&str>,
) -> AppResult<Session> {
    get_game(conn, game_id)?;
    let duration = session_duration(started_at, ended_at, 0, started_at);
    conn.execute(
        "INSERT INTO sessions (game_id, started_at, ended_at, duration_secs, source, note)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            game_id,
            started_at,
            ended_at,
            duration,
            source.as_str(),
            note
        ],
    )?;
    get_session(conn, conn.last_insert_rowid() as i32, started_at)
}

/// Closes a session at `ended_at` (never before it started) and drops its heartbeat.
pub fn end_session(conn: &Connection, id: i32, ended_at: i64) -> AppResult<Session> {
    let s = get_session(conn, id, ended_at)?;
    let ended_at = ended_at.max(s.started_at);
    let duration = session_duration(s.started_at, Some(ended_at), s.afk_secs, ended_at);
    conn.execute(
        "UPDATE sessions SET ended_at = ?1, duration_secs = ?2 WHERE id = ?3",
        params![ended_at, duration, id],
    )?;
    conn.execute("DELETE FROM session_heartbeats WHERE session_id = ?1", [id])?;
    get_session(conn, id, ended_at)
}

pub fn reopen_session(conn: &Connection, id: i32, now: i64) -> AppResult<()> {
    conn.execute("UPDATE sessions SET ended_at = NULL WHERE id = ?1", [id])?;
    write_heartbeat(conn, id, now)
}

pub fn write_heartbeat(conn: &Connection, session_id: i32, at: i64) -> AppResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO session_heartbeats (session_id, last_seen_at) VALUES (?1, ?2)",
        params![session_id, at],
    )?;
    Ok(())
}

pub fn last_heartbeat(conn: &Connection, session_id: i32) -> AppResult<Option<i64>> {
    Ok(conn
        .query_row(
            "SELECT last_seen_at FROM session_heartbeats WHERE session_id = ?1",
            [session_id],
            |r| r.get(0),
        )
        .optional()?)
}

pub fn active_session_for_game(conn: &Connection, game_id: i32) -> AppResult<Option<Session>> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {SESSION_COLS} FROM sessions s WHERE s.game_id = ?1 AND s.ended_at IS NULL LIMIT 1"
            ),
            [game_id],
            |r| map_session(r, 0),
        )
        .optional()?)
}

/// The most recently ended auto session of a game, used for merging.
pub fn last_ended_auto_session(conn: &Connection, game_id: i32) -> AppResult<Option<Session>> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {SESSION_COLS} FROM sessions s
                 WHERE s.game_id = ?1 AND s.ended_at IS NOT NULL AND s.source = 'auto'
                 ORDER BY s.ended_at DESC, s.id DESC LIMIT 1"
            ),
            [game_id],
            |r| map_session(r, 0),
        )
        .optional()?)
}

pub fn list_sessions(
    conn: &Connection,
    q: &SessionQuery,
    now: i64,
) -> AppResult<Vec<SessionWithGame>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SESSION_COLS}, g.name FROM sessions s JOIN games g ON g.id = s.game_id
         WHERE (?1 IS NULL OR s.game_id = ?1)
           AND (?2 IS NULL OR s.started_at >= ?2)
           AND (?3 IS NULL OR s.started_at < ?3)
         ORDER BY s.started_at DESC, s.id DESC
         LIMIT ?4 OFFSET ?5"
    ))?;
    let rows = stmt.query_map(
        params![q.game_id, q.from, q.to, q.limit.max(1), q.offset],
        |r| {
            Ok(SessionWithGame {
                session: map_session(r, now)?,
                game_name: r.get(7)?,
            })
        },
    )?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn create_session(
    conn: &Connection,
    game_id: i32,
    started_at: i64,
    ended_at: i64,
    note: Option<&str>,
) -> AppResult<Session> {
    if ended_at <= started_at {
        return Err(err("endedAt must be after startedAt"));
    }
    let note = note.filter(|n| !n.is_empty());
    insert_session(
        conn,
        game_id,
        started_at,
        Some(ended_at),
        SessionSource::Manual,
        note,
    )
}

pub fn update_session(
    conn: &Connection,
    id: i32,
    patch: &SessionUpdate,
    now: i64,
) -> AppResult<Session> {
    let s = get_session(conn, id, now)?;
    let Some(old_end) = s.ended_at else {
        return Err(err("session is active; stop it first"));
    };
    let started_at = patch.started_at.unwrap_or(s.started_at);
    let ended_at = patch.ended_at.unwrap_or(old_end);
    if ended_at <= started_at {
        return Err(err("endedAt must be after startedAt"));
    }
    let note = match &patch.note {
        None => s.note.clone(),
        Some(n) if n.is_empty() => None,
        Some(n) => Some(n.clone()),
    };
    let duration = session_duration(started_at, Some(ended_at), s.afk_secs, ended_at);
    conn.execute(
        "UPDATE sessions SET started_at = ?1, ended_at = ?2, duration_secs = ?3, note = ?4 WHERE id = ?5",
        params![started_at, ended_at, duration, note, id],
    )?;
    get_session(conn, id, now)
}

pub fn delete_session(conn: &Connection, id: i32, now: i64) -> AppResult<()> {
    let s = get_session(conn, id, now)?;
    if s.ended_at.is_none() {
        return Err(err("session is active; stop it first"));
    }
    conn.execute("DELETE FROM sessions WHERE id = ?1", [id])?;
    Ok(())
}

/// Crash recovery: closes every open session at its last heartbeat (or `startedAt`).
/// Returns the number of sessions closed.
pub fn recover_open_sessions(conn: &Connection) -> AppResult<usize> {
    let open: Vec<(i32, i64, Option<i64>)> = {
        let mut stmt = conn.prepare(
            "SELECT s.id, s.started_at, h.last_seen_at FROM sessions s
             LEFT JOIN session_heartbeats h ON h.session_id = s.id
             WHERE s.ended_at IS NULL",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        rows.collect::<Result<_, _>>()?
    };
    for (id, started_at, beat) in &open {
        end_session(conn, *id, beat.unwrap_or(*started_at))?;
    }
    Ok(open.len())
}
