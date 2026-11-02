//! Daily playtime limits: storage, validation, usage and the "already notified" state.
//! No Tauri types: the tracker returns the statuses to notify and `crate::runtime` emits them.

use std::collections::HashMap;

use chrono::{Duration, NaiveDate};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::core::db::{repo, stats};
use crate::core::types::{Limit, LimitAction, LimitInput, LimitScope, LimitStatus};
use crate::error::{AppError, AppResult};

pub const DAILY_MIN: i64 = 60;
pub const DAILY_MAX: i64 = 86_400;
/// How often `notify_repeat` fires again while a counted session keeps running.
pub const REPEAT_SECS: i64 = 15 * 60;

fn err(msg: impl Into<String>) -> AppError {
    AppError::Other(msg.into())
}

const SELECT: &str = "SELECT l.id, l.scope, l.game_id, g.name, l.daily_secs, l.action
                      FROM limits l LEFT JOIN games g ON g.id = l.game_id";

fn map_limit(row: &Row) -> rusqlite::Result<Limit> {
    let scope: String = row.get(1)?;
    let action: String = row.get(5)?;
    Ok(Limit {
        id: row.get(0)?,
        scope: LimitScope::parse(&scope).unwrap_or(LimitScope::Global),
        game_id: row.get(2)?,
        game_name: row.get(3)?,
        daily_secs: row.get(4)?,
        action: LimitAction::parse(&action).unwrap_or(LimitAction::Notify),
    })
}

pub fn get_limit(conn: &Connection, id: i32) -> AppResult<Limit> {
    conn.query_row(&format!("{SELECT} WHERE l.id = ?1"), [id], map_limit)
        .optional()?
        .ok_or_else(|| err(format!("limit {id} not found")))
}

/// Global first, then games by name.
pub fn list_limits(conn: &Connection) -> AppResult<Vec<Limit>> {
    let mut stmt = conn.prepare(&format!(
        "{SELECT} ORDER BY (l.scope = 'game'), lower(g.name), l.id"
    ))?;
    let rows = stmt.query_map([], map_limit)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn validate(conn: &Connection, input: &LimitInput, own_id: Option<i32>) -> AppResult<()> {
    if !(DAILY_MIN..=DAILY_MAX).contains(&input.daily_secs) {
        return Err(err(format!(
            "dailySecs must be between {DAILY_MIN} and {DAILY_MAX}"
        )));
    }
    match (input.scope, input.game_id) {
        (LimitScope::Global, Some(_)) => {
            return Err(err("a global limit must not have a gameId"));
        }
        (LimitScope::Game, None) => return Err(err("a game limit needs a gameId")),
        (LimitScope::Game, Some(id)) => {
            repo::get_game(conn, id)?;
        }
        (LimitScope::Global, None) => {}
    }
    let duplicate: Option<i32> = conn
        .query_row(
            "SELECT id FROM limits
             WHERE scope = ?1 AND game_id IS ?2 AND (?3 IS NULL OR id != ?3) LIMIT 1",
            params![input.scope.as_str(), input.game_id, own_id],
            |r| r.get(0),
        )
        .optional()?;
    if duplicate.is_some() {
        return Err(err(match input.scope {
            LimitScope::Global => "a global limit already exists",
            LimitScope::Game => "that game already has a limit",
        }));
    }
    Ok(())
}

pub fn create_limit(conn: &Connection, input: &LimitInput) -> AppResult<Limit> {
    validate(conn, input, None)?;
    conn.execute(
        "INSERT INTO limits (scope, game_id, daily_secs, action) VALUES (?1, ?2, ?3, ?4)",
        params![
            input.scope.as_str(),
            input.game_id,
            input.daily_secs,
            input.action.as_str()
        ],
    )?;
    get_limit(conn, conn.last_insert_rowid() as i32)
}

pub fn update_limit(conn: &Connection, id: i32, input: &LimitInput) -> AppResult<Limit> {
    get_limit(conn, id)?;
    validate(conn, input, Some(id))?;
    conn.execute(
        "UPDATE limits SET scope = ?1, game_id = ?2, daily_secs = ?3, action = ?4 WHERE id = ?5",
        params![
            input.scope.as_str(),
            input.game_id,
            input.daily_secs,
            input.action.as_str(),
            id
        ],
    )?;
    get_limit(conn, id)
}

pub fn delete_limit(conn: &Connection, id: i32) -> AppResult<()> {
    get_limit(conn, id)?;
    conn.execute("DELETE FROM limits WHERE id = ?1", [id])?;
    Ok(())
}

/// Usage of one limit today (local midnight to midnight), active sessions included, AFK excluded.
pub fn status_for(conn: &Connection, limit: Limit, now: i64) -> AppResult<LimitStatus> {
    let today = stats::local_date(now);
    let used = stats::range_secs(
        conn,
        stats::local_midnight(today),
        stats::local_midnight(today + Duration::days(1)),
        now,
        limit.game_id,
    )?;
    Ok(LimitStatus {
        used_today_secs: used,
        remaining_secs: (limit.daily_secs - used).max(0),
        reached: used >= limit.daily_secs,
        limit,
    })
}

pub fn list_statuses(conn: &Connection, now: i64) -> AppResult<Vec<LimitStatus>> {
    list_limits(conn)?
        .into_iter()
        .map(|l| status_for(conn, l, now))
        .collect()
}

/// In-memory "already notified today" state. Resets at local midnight and on restart.
#[derive(Default)]
pub struct LimitMonitor {
    day: Option<NaiveDate>,
    /// Limit id to the time of its last notification.
    notified: HashMap<i32, i64>,
}

impl LimitMonitor {
    /// Returns the limits to notify about on this poll.
    ///
    /// `counting` holds the games with a running, non-AFK session. A limit is only judged while
    /// a session counted against it runs, so a limit that was already exceeded earlier today
    /// does not fire at startup with nothing running.
    pub fn evaluate(
        &mut self,
        conn: &Connection,
        now: i64,
        counting: &[i32],
    ) -> AppResult<Vec<LimitStatus>> {
        let today = stats::local_date(now);
        if self.day != Some(today) {
            self.day = Some(today);
            self.notified.clear();
        }
        let limits = list_limits(conn)?;
        self.notified
            .retain(|id, _| limits.iter().any(|l| l.id == *id));
        if counting.is_empty() {
            return Ok(Vec::new());
        }
        let mut counting_visible = false;
        for id in counting {
            if !repo::get_game(conn, *id)?.hidden {
                counting_visible = true;
                break;
            }
        }
        let mut due = Vec::new();
        for limit in limits {
            let counted = match limit.game_id {
                Some(id) => counting.contains(&id),
                // A global limit is only judged while a non-hidden game is counting.
                None => counting_visible,
            };
            if !counted {
                continue;
            }
            let id = limit.id;
            let action = limit.action;
            let status = status_for(conn, limit, now)?;
            if !status.reached {
                // Raised or reset: it may fire again once it is reached again.
                self.notified.remove(&id);
                continue;
            }
            let fire = match self.notified.get(&id) {
                None => true,
                Some(last) => action == LimitAction::NotifyRepeat && now - last >= REPEAT_SECS,
            };
            if fire {
                self.notified.insert(id, now);
                due.push(status);
            }
        }
        Ok(due)
    }
}
