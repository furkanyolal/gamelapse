mod common;

use chrono::NaiveDate;
use common::{add_game, test_db};
use gamelapse_lib::core::db::{repo, seed, stats};
use gamelapse_lib::core::types::{MatchMode, SessionSource};

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

#[test]
fn daily_totals_split_a_session_across_midnight() {
    let conn = test_db();
    let game = add_game(&conn, "Foo", r"c:\foo.exe", MatchMode::ExactPath);
    let day1 = date(2026, 6, 10);
    let day2 = date(2026, 6, 11);
    let midnight = stats::local_midnight(day2);
    // 23:00 on day 1 to 02:00 on day 2.
    repo::insert_session(
        &conn,
        game,
        midnight - 3600,
        Some(midnight + 7200),
        SessionSource::Manual,
        None,
    )
    .unwrap();

    let now = midnight + 10 * 3600;
    let totals = stats::daily_totals(&conn, now, 3, None).unwrap();
    let got: Vec<(&str, i64)> = totals
        .iter()
        .map(|t| (t.date.as_str(), t.total_secs))
        .collect();
    // Oldest first, zero-filled: the 9th is empty.
    assert_eq!(
        got,
        vec![
            ("2026-06-09", 0),
            ("2026-06-10", 3600),
            ("2026-06-11", 7200)
        ]
    );
    assert_eq!(day1.format("%Y-%m-%d").to_string(), "2026-06-10");

    let other = add_game(&conn, "Bar", r"c:\bar.exe", MatchMode::ExactPath);
    let only_other = stats::daily_totals(&conn, now, 3, Some(other)).unwrap();
    assert!(only_other.iter().all(|t| t.total_secs == 0));
}

#[test]
fn hourly_totals_split_a_session_across_hours() {
    let conn = test_db();
    let game = add_game(&conn, "Foo", r"c:\foo.exe", MatchMode::ExactPath);
    let midnight = stats::local_midnight(date(2026, 6, 10));
    // 09:30 to 11:15.
    repo::insert_session(
        &conn,
        game,
        midnight + 9 * 3600 + 1800,
        Some(midnight + 11 * 3600 + 900),
        SessionSource::Manual,
        None,
    )
    .unwrap();

    let hourly = stats::hourly_totals(&conn, midnight + 20 * 3600, 1).unwrap();
    assert_eq!(hourly.len(), 24);
    for h in &hourly {
        let expected = match h.hour {
            9 => 1800,
            10 => 3600,
            11 => 900,
            _ => 0,
        };
        assert_eq!(h.total_secs, expected, "hour {}", h.hour);
    }
}

#[test]
fn active_sessions_count_up_to_now() {
    let conn = test_db();
    let game = add_game(&conn, "Foo", r"c:\foo.exe", MatchMode::ExactPath);
    let midnight = stats::local_midnight(date(2026, 6, 10));
    repo::insert_session(
        &conn,
        game,
        midnight + 3600,
        None,
        SessionSource::Manual,
        None,
    )
    .unwrap();
    let now = midnight + 3600 + 600;
    let dash = stats::dashboard(&conn, now).unwrap();
    assert_eq!(dash.today_secs, 600);
    assert_eq!(dash.all_time_secs, 600);
    assert_eq!(dash.recent_sessions[0].session.duration_secs, 600);
    assert_eq!(dash.recent_sessions[0].session.ended_at, None);
}

#[test]
fn seeded_dashboard_matches_a_direct_sum() {
    let conn = test_db();
    let now = stats::local_midnight(date(2026, 6, 10)) + 14 * 3600;
    assert!(seed::seed_demo_data(&conn, now).unwrap());
    assert!(
        !seed::seed_demo_data(&conn, now).unwrap(),
        "second seed is a no-op"
    );

    let games: i64 = conn
        .query_row("SELECT COUNT(*) FROM games", [], |r| r.get(0))
        .unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
        .unwrap();
    assert_eq!((games, count), (8, 120));

    let direct: i64 = conn
        .query_row(
            "SELECT SUM(ended_at - started_at - afk_secs) FROM sessions",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let played: i64 = conn
        .query_row("SELECT COUNT(DISTINCT game_id) FROM sessions", [], |r| {
            r.get(0)
        })
        .unwrap();

    let dash = stats::dashboard(&conn, now).unwrap();
    assert_eq!(dash.all_time_secs, direct);
    assert_eq!(i64::from(dash.games_played_count), played);
    assert_eq!(dash.recent_sessions.len(), 10);
    assert!(dash.top_games.len() <= 5);
    assert!(
        dash.top_games
            .windows(2)
            .all(|w| w[0].total_secs >= w[1].total_secs)
    );

    // Summing every game's total equals the all-time total.
    let summaries = stats::list_game_summaries(&conn, true, now).unwrap();
    assert_eq!(summaries.iter().map(|s| s.total_secs).sum::<i64>(), direct);
    assert_eq!(
        summaries
            .iter()
            .map(|s| i64::from(s.session_count))
            .sum::<i64>(),
        count
    );

    // The week figure equals the last seven daily buckets; today equals the last bucket.
    let daily = stats::daily_totals(&conn, now, 7, None).unwrap();
    assert_eq!(
        dash.week_secs,
        daily.iter().map(|d| d.total_secs).sum::<i64>()
    );
    assert_eq!(dash.today_secs, daily.last().unwrap().total_secs);

    // Hourly totals over the whole seeded range cover all session time before now.
    let hourly = stats::hourly_totals(&conn, now, 61).unwrap();
    assert_eq!(hourly.iter().map(|h| h.total_secs).sum::<i64>(), direct);
}

#[test]
fn baseline_counts_toward_all_time_only() {
    let conn = test_db();
    let game = add_game(&conn, "Foo", r"c:\foo.exe", MatchMode::ExactPath);
    let other = add_game(&conn, "Bar", r"c:\bar.exe", MatchMode::ExactPath);
    let midnight = stats::local_midnight(date(2026, 6, 10));
    repo::insert_session(
        &conn,
        game,
        midnight + 3600,
        Some(midnight + 3600 + 1800),
        SessionSource::Manual,
        None,
    )
    .unwrap();
    conn.execute(
        "INSERT INTO imported_playtime (game_id, source, minutes, fetched_at) VALUES (?1, 'steam', 60, 0)",
        [game],
    )
    .unwrap();
    // A game with only a baseline still counts as played.
    conn.execute(
        "INSERT INTO imported_playtime (game_id, source, minutes, fetched_at) VALUES (?1, 'steam', 10, 0)",
        [other],
    )
    .unwrap();
    let now = midnight + 4 * 3600;

    let summaries = stats::list_game_summaries(&conn, true, now).unwrap();
    let s = summaries.iter().find(|s| s.game.id == game).unwrap();
    assert_eq!(
        (s.total_secs, s.today_secs, s.imported_minutes),
        (5400, 1800, 60)
    );

    let dash = stats::dashboard(&conn, now).unwrap();
    assert_eq!(dash.all_time_secs, 5400 + 600);
    assert_eq!(dash.today_secs, 1800);
    assert_eq!(dash.games_played_count, 2);
    assert_eq!(dash.top_games[0].total_secs, 5400);

    let all = stats::game_totals(&conn, now, None, None, 5).unwrap();
    assert_eq!(
        all.iter().map(|g| g.total_secs).collect::<Vec<_>>(),
        vec![5400, 600]
    );
    let bounded =
        stats::game_totals(&conn, now, Some(midnight), Some(midnight + 86400), 5).unwrap();
    assert_eq!(bounded.len(), 1);
    assert_eq!(bounded[0].total_secs, 1800);
}
