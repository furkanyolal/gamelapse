//! Aggregate queries. Day and hour boundaries use the local time zone.
//! Active sessions count up to `now`.

use chrono::{Duration, Local, NaiveDate, TimeZone, Timelike};
use rusqlite::{Connection, params};

use crate::core::db::{library, repo};
use crate::core::types::{
    DailyTotal, DashboardStats, Game, GameSummary, GameTotal, HourlyTotal, SessionQuery, Tag,
};
use crate::error::AppResult;

/// Local date containing the unix timestamp.
pub fn local_date(ts: i64) -> NaiveDate {
    Local
        .timestamp_opt(ts, 0)
        .earliest()
        .map(|d| d.date_naive())
        .unwrap_or_else(|| Local::now().date_naive())
}

/// Unix timestamp of local midnight at the start of `date`.
pub fn local_midnight(date: NaiveDate) -> i64 {
    let midnight = date.and_hms_opt(0, 0, 0).expect("valid time");
    Local
        .from_local_datetime(&midnight)
        .earliest()
        // Midnight does not exist on some DST transitions; the next hour does.
        .or_else(|| {
            Local
                .from_local_datetime(&(midnight + Duration::hours(1)))
                .earliest()
        })
        .map(|d| d.timestamp())
        .unwrap_or_else(|| midnight.and_utc().timestamp())
}

/// A session reduced to what aggregation needs: its wall-clock span and AFK seconds.
struct Span {
    start: i64,
    end: i64,
    afk: i64,
}

impl Span {
    /// Counted seconds of a wall-clock piece, scaled down by the session's AFK share.
    fn scale(&self, piece: i64) -> i64 {
        let wall = self.end - self.start;
        if piece <= 0 || wall <= 0 {
            return 0;
        }
        if self.afk <= 0 {
            return piece;
        }
        let eff = (wall - self.afk).max(0);
        piece * eff / wall
    }

    /// Counted seconds inside `[from, to)`.
    fn clipped(&self, from: i64, to: i64) -> i64 {
        let s = self.start.max(from);
        let e = self.end.min(to);
        self.scale(e - s)
    }
}

fn load_spans(
    conn: &Connection,
    from: i64,
    to: i64,
    now: i64,
    game_id: Option<i32>,
) -> AppResult<Vec<Span>> {
    let mut stmt = conn.prepare(
        "SELECT started_at, COALESCE(ended_at, ?1), afk_secs FROM sessions
         WHERE started_at < ?3 AND COALESCE(ended_at, ?1) > ?2 AND (?4 IS NULL OR game_id = ?4)
           AND (?4 IS NOT NULL OR game_id IN (SELECT id FROM games WHERE hidden = 0))",
    )?;
    let rows = stmt.query_map(params![now, from, to, game_id], |r| {
        Ok(Span {
            start: r.get(0)?,
            end: r.get(1)?,
            afk: r.get(2)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Counted seconds inside `[from, to)` across all non-hidden games' sessions, or one game's
/// (hidden or not).
pub fn range_secs(
    conn: &Connection,
    from: i64,
    to: i64,
    now: i64,
    game_id: Option<i32>,
) -> AppResult<i64> {
    Ok(load_spans(conn, from, to, now, game_id)?
        .iter()
        .map(|s| s.clipped(from, to))
        .sum())
}

fn today_bounds(now: i64) -> (i64, i64, i64) {
    let today = local_date(now);
    (
        local_midnight(today - Duration::days(6)),
        local_midnight(today),
        local_midnight(today + Duration::days(1)),
    )
}

pub fn daily_totals(
    conn: &Connection,
    now: i64,
    days: u32,
    game_id: Option<i32>,
) -> AppResult<Vec<DailyTotal>> {
    let days = days.max(1) as i64;
    let today = local_date(now);
    let dates: Vec<NaiveDate> = (0..days).rev().map(|i| today - Duration::days(i)).collect();
    let mut bounds: Vec<i64> = dates.iter().map(|d| local_midnight(*d)).collect();
    bounds.push(local_midnight(today + Duration::days(1)));

    let spans = load_spans(conn, bounds[0], bounds[bounds.len() - 1], now, game_id)?;
    let mut totals = vec![0i64; dates.len()];
    for span in &spans {
        for (i, total) in totals.iter_mut().enumerate() {
            if span.start >= bounds[i + 1] || span.end <= bounds[i] {
                continue;
            }
            *total += span.clipped(bounds[i], bounds[i + 1]);
        }
    }
    Ok(dates
        .iter()
        .zip(totals)
        .map(|(d, total_secs)| DailyTotal {
            date: d.format("%Y-%m-%d").to_string(),
            total_secs,
        })
        .collect())
}

/// Local hour-of-day totals over the last `days` local days including today.
pub fn hourly_totals(conn: &Connection, now: i64, days: u32) -> AppResult<Vec<HourlyTotal>> {
    let days = days.max(1) as i64;
    let today = local_date(now);
    let from = local_midnight(today - Duration::days(days - 1));
    let to = local_midnight(today + Duration::days(1));

    let mut buckets = [0i64; 24];
    for span in load_spans(conn, from, to, now, None)? {
        let end = span.end.min(to);
        let mut t = span.start.max(from);
        while t < end {
            let Some(dt) = Local.timestamp_opt(t, 0).earliest() else {
                break;
            };
            let into_hour = (dt.minute() * 60 + dt.second()) as i64;
            let cut = (t + 3600 - into_hour).min(end);
            buckets[dt.hour() as usize] += span.scale(cut - t);
            t = cut;
        }
    }
    Ok(buckets
        .iter()
        .enumerate()
        .map(|(hour, total_secs)| HourlyTotal {
            hour: hour as i32,
            total_secs: *total_secs,
        })
        .collect())
}

pub fn game_totals(
    conn: &Connection,
    now: i64,
    from: Option<i64>,
    to: Option<i64>,
    limit: u32,
) -> AppResult<Vec<GameTotal>> {
    // Launcher baselines belong to "all time", so only an unbounded query includes them.
    let with_baseline = from.is_none() && to.is_none();
    let mut stmt = conn.prepare(
        "SELECT g.id, g.name,
                COALESCE(t.total, 0) + CASE WHEN ?5 THEN COALESCE(b.minutes, 0) * 60 ELSE 0 END AS grand
         FROM games g
         LEFT JOIN (
             SELECT game_id,
                    SUM(MAX(0, COALESCE(ended_at, ?1) - started_at - afk_secs)) AS total
             FROM sessions
             WHERE (?2 IS NULL OR started_at >= ?2) AND (?3 IS NULL OR started_at < ?3)
             GROUP BY game_id
         ) t ON t.game_id = g.id
         LEFT JOIN (
             SELECT game_id, SUM(minutes) AS minutes FROM imported_playtime GROUP BY game_id
         ) b ON b.game_id = g.id
         WHERE g.hidden = 0 AND grand > 0
         ORDER BY grand DESC, g.sort_name
         LIMIT ?4",
    )?;
    let rows = stmt.query_map(params![now, from, to, limit.max(1), with_baseline], |r| {
        Ok(GameTotal {
            game_id: r.get(0)?,
            game_name: r.get(1)?,
            total_secs: r.get(2)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn dashboard(conn: &Connection, now: i64) -> AppResult<DashboardStats> {
    let (week_start, today_start, tomorrow) = today_bounds(now);
    let tracked_secs: i64 = conn.query_row(
        "SELECT COALESCE(SUM(MAX(0, COALESCE(ended_at, ?1) - started_at - afk_secs)), 0)
         FROM sessions WHERE game_id IN (SELECT id FROM games WHERE hidden = 0)",
        [now],
        |r| r.get(0),
    )?;
    let baseline_minutes: i64 = conn.query_row(
        "SELECT COALESCE(SUM(minutes), 0) FROM imported_playtime
         WHERE game_id IN (SELECT id FROM games WHERE hidden = 0)",
        [],
        |r| r.get(0),
    )?;
    let games_played_count: i32 = conn.query_row(
        "SELECT COUNT(*) FROM games
         WHERE hidden = 0
           AND (id IN (SELECT game_id FROM sessions)
                OR id IN (SELECT game_id FROM imported_playtime WHERE minutes > 0))",
        [],
        |r| r.get(0),
    )?;
    let all_time_secs = tracked_secs + baseline_minutes * 60;
    Ok(DashboardStats {
        today_secs: range_secs(conn, today_start, tomorrow, now, None)?,
        week_secs: range_secs(conn, week_start, tomorrow, now, None)?,
        all_time_secs,
        games_played_count,
        top_games: game_totals(conn, now, None, None, 5)?,
        recent_sessions: repo::list_sessions(
            conn,
            &SessionQuery {
                game_id: None,
                from: None,
                to: None,
                limit: 10,
                offset: 0,
                include_hidden: false,
            },
            now,
        )?,
    })
}

pub fn game_summary(conn: &Connection, game: Game, now: i64) -> AppResult<GameSummary> {
    let tags = library::tags_for_game(conn, game.id)?;
    summary_with_tags(conn, game, tags, now)
}

fn summary_with_tags(
    conn: &Connection,
    game: Game,
    tags: Vec<Tag>,
    now: i64,
) -> AppResult<GameSummary> {
    let (week_start, today_start, tomorrow) = today_bounds(now);
    let (total_secs, session_count, last_played_at): (i64, i32, Option<i64>) = conn.query_row(
        "SELECT COALESCE(SUM(MAX(0, COALESCE(ended_at, ?1) - started_at - afk_secs)), 0),
                COUNT(*), MAX(started_at)
         FROM sessions WHERE game_id = ?2",
        params![now, game.id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let imported_minutes: i64 = conn.query_row(
        "SELECT COALESCE(SUM(minutes), 0) FROM imported_playtime WHERE game_id = ?1",
        [game.id],
        |r| r.get(0),
    )?;
    let id = Some(game.id);
    Ok(GameSummary {
        today_secs: range_secs(conn, today_start, tomorrow, now, id)?,
        week_secs: range_secs(conn, week_start, tomorrow, now, id)?,
        tags,
        game,
        total_secs: total_secs + imported_minutes * 60,
        session_count,
        last_played_at,
        imported_minutes,
    })
}

pub fn list_game_summaries(
    conn: &Connection,
    include_hidden: bool,
    now: i64,
) -> AppResult<Vec<GameSummary>> {
    // One query for every game's tags instead of one per game.
    let mut tags = library::tags_by_game(conn)?;
    repo::list_games(conn, include_hidden)?
        .into_iter()
        .map(|g| {
            let game_tags = tags.remove(&g.id).unwrap_or_default();
            summary_with_tags(conn, g, game_tags, now)
        })
        .collect()
}
