//! Deterministic demo dataset for development and screenshots.

use chrono::Duration;
use rusqlite::Connection;

use crate::core::db::{repo, stats};
use crate::core::types::{GameSource, SessionSource};
use crate::error::AppResult;

const DEMO_GAMES: [&str; 8] = [
    "Hollow Knight",
    "The Witcher 3",
    "Factorio",
    "Elden Ring",
    "Stardew Valley",
    "Counter-Strike 2",
    "Hades",
    "Celeste",
];
const SESSION_COUNT: usize = 120;
const DAYS: i64 = 60;

/// Small linear congruential generator; the seed is fixed so every run inserts the same data.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, modulo: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) % modulo
    }
}

fn demo_exists(conn: &Connection) -> AppResult<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM games WHERE source_id IN
         ('demo-1','demo-2','demo-3','demo-4','demo-5','demo-6','demo-7','demo-8')",
        [],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

/// Inserts the demo games and sessions relative to `now`. Returns false (and changes nothing)
/// if any demo game exists.
pub fn seed_demo_data(conn: &Connection, now: i64) -> AppResult<bool> {
    if demo_exists(conn)? {
        return Ok(false);
    }
    let tx = conn.unchecked_transaction()?;
    let mut game_ids = Vec::new();
    for (i, name) in DEMO_GAMES.iter().enumerate() {
        let g = repo::insert_game(
            &tx,
            name,
            GameSource::Manual,
            Some(&format!("demo-{}", i + 1)),
            None,
            now,
        )?;
        game_ids.push(g.id);
    }

    let today = stats::local_date(now);
    let mut rng = Lcg(0x005E_ED0F_6A4E);
    for n in 0..SESSION_COUNT {
        // Min of two draws favours the first games, so the totals are uneven.
        let game = game_ids[rng.next(8).min(rng.next(8)) as usize];
        let day_offset = rng.next(DAYS as u64) as i64;
        let start_minute = 6 * 60 + rng.next(18 * 60) as i64;
        let length_mins = 10 + rng.next(291) as i64;
        let mut started =
            stats::local_midnight(today - Duration::days(day_offset)) + start_minute * 60;
        let mut ended = started + length_mins * 60;
        if ended > now {
            started -= 86_400;
            ended -= 86_400;
        }
        let source = if n % 5 == 4 {
            SessionSource::Manual
        } else {
            SessionSource::Auto
        };
        repo::insert_session(&tx, game, started, Some(ended), source, None)?;
    }
    tx.commit()?;
    Ok(true)
}
