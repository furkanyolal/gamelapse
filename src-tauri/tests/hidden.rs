mod common;

use std::collections::HashMap;

use chrono::NaiveDate;
use common::{BAR_EXE, FOO_EXE, TrackRig, add_game, test_db};
use gamelapse_lib::core::db::{migrations, repo, stats};
use gamelapse_lib::core::importers::steam::KNOWN_TOOLS;
use gamelapse_lib::core::importers::store;
use gamelapse_lib::core::limits;
use gamelapse_lib::core::tracker::TrackerEvent;
use gamelapse_lib::core::types::{
    DiscoveredGame, GameUpdate, ImportSource, LimitAction, LimitInput, LimitScope, MatchMode,
    SessionQuery, SessionSource,
};
use rusqlite::Connection;

fn midnight(day: u32) -> i64 {
    stats::local_midnight(NaiveDate::from_ymd_opt(2026, 7, day).unwrap())
}

fn hide(conn: &Connection, id: i32, hidden: bool) {
    repo::update_game(
        conn,
        id,
        &GameUpdate {
            name: None,
            hidden: Some(hidden),
            afk_detection_enabled: None,
        },
    )
    .unwrap();
}

/// A visible game "shown_game" and a hidden game "Tool", each with one hour today and a 10 h baseline
/// on the hidden one. Returns `(conn, shown_game, tool, now)`.
fn world() -> (Connection, i32, i32, i64) {
    let conn = test_db();
    let shown_game = add_game(&conn, "shown_game", FOO_EXE, MatchMode::ExactPath);
    let tool = add_game(&conn, "Tool", BAR_EXE, MatchMode::ExactPath);
    let day = midnight(10);
    for (game, start) in [(shown_game, day + 9 * 3600), (tool, day + 12 * 3600)] {
        repo::insert_session(
            &conn,
            game,
            start,
            Some(start + 3600),
            SessionSource::Manual,
            None,
        )
        .unwrap();
    }
    conn.execute(
        "INSERT INTO imported_playtime (game_id, source, minutes, fetched_at) VALUES (?1, 'steam', 600, 0)",
        [tool],
    )
    .unwrap();
    hide(&conn, tool, true);
    (conn, shown_game, tool, day + 20 * 3600)
}

fn query(include_hidden: bool) -> SessionQuery {
    SessionQuery {
        game_id: None,
        from: None,
        to: None,
        limit: 100,
        offset: 0,
        include_hidden,
    }
}

#[test]
fn dashboard_excludes_hidden_games() {
    let (conn, shown_game, _tool, now) = world();
    let d = stats::dashboard(&conn, now).unwrap();
    assert_eq!(d.today_secs, 3600);
    assert_eq!(d.week_secs, 3600);
    assert_eq!(d.all_time_secs, 3600);
    assert_eq!(d.games_played_count, 1);
    assert_eq!(d.top_games.len(), 1);
    assert_eq!(d.top_games[0].game_id, shown_game);
    assert_eq!(d.recent_sessions.len(), 1);
    assert_eq!(d.recent_sessions[0].session.game_id, shown_game);
}

#[test]
fn daily_hourly_and_game_totals_exclude_hidden_games() {
    let (conn, shown_game, tool, now) = world();
    let daily = stats::daily_totals(&conn, now, 1, None).unwrap();
    assert_eq!(daily[0].total_secs, 3600);
    // A game's own view still shows everything.
    let own = stats::daily_totals(&conn, now, 1, Some(tool)).unwrap();
    assert_eq!(own[0].total_secs, 3600);

    let hourly = stats::hourly_totals(&conn, now, 1).unwrap();
    assert_eq!(hourly.iter().map(|h| h.total_secs).sum::<i64>(), 3600);
    assert_eq!(hourly[12].total_secs, 0);
    assert_eq!(hourly[9].total_secs, 3600);

    let day = midnight(10);
    let bounded = stats::game_totals(&conn, now, Some(day), Some(day + 86_400), 10).unwrap();
    assert_eq!(
        bounded.iter().map(|g| g.game_id).collect::<Vec<_>>(),
        [shown_game]
    );
    // Unbounded totals would add the 10 h baseline for the hidden game.
    let unbounded = stats::game_totals(&conn, now, None, None, 10).unwrap();
    assert_eq!(
        unbounded.iter().map(|g| g.game_id).collect::<Vec<_>>(),
        [shown_game]
    );
}

#[test]
fn list_sessions_hides_hidden_games_unless_asked() {
    let (conn, shown_game, _tool, now) = world();
    let shown = repo::list_sessions(&conn, &query(false), now).unwrap();
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].session.game_id, shown_game);
    assert_eq!(
        repo::list_sessions(&conn, &query(true), now).unwrap().len(),
        2
    );
}

#[test]
fn session_query_include_hidden_defaults_to_false() {
    let q: SessionQuery =
        serde_json::from_str(r#"{"gameId":null,"from":null,"to":null,"limit":5,"offset":0}"#)
            .unwrap();
    assert!(!q.include_hidden);
}

#[test]
fn global_limit_ignores_hidden_games_but_a_game_limit_on_one_still_reaches() {
    let mut rig = TrackRig::new();
    let tool = add_game(&rig.conn, "Tool", BAR_EXE, MatchMode::ExactPath);
    hide(&rig.conn, tool, true);
    rig.source.set(&[BAR_EXE], None);
    let global = limits::create_limit(
        &rig.conn,
        &LimitInput {
            scope: LimitScope::Global,
            game_id: None,
            daily_secs: 600,
            action: LimitAction::Notify,
        },
    )
    .unwrap();
    let per_game = limits::create_limit(
        &rig.conn,
        &LimitInput {
            scope: LimitScope::Game,
            game_id: Some(tool),
            daily_secs: 600,
            action: LimitAction::Notify,
        },
    )
    .unwrap();

    let t0 = midnight(10) + 10 * 3600;
    rig.tick(t0, 0);
    let events = rig.tick(t0 + 900, 0);
    let fired: Vec<i32> = events
        .iter()
        .filter_map(|e| match e {
            TrackerEvent::LimitReached(s) => Some(s.limit.id),
            _ => None,
        })
        .collect();
    assert_eq!(fired, vec![per_game.id]);

    let global_status = limits::status_for(&rig.conn, global, t0 + 900).unwrap();
    assert_eq!(global_status.used_today_secs, 0);
    assert!(!global_status.reached);
    let game_status = limits::status_for(&rig.conn, per_game, t0 + 900).unwrap();
    assert_eq!(game_status.used_today_secs, 900);
    assert!(game_status.reached);
}

#[test]
fn reminders_skip_hidden_games_and_active_session_carries_the_flag() {
    let mut rig = TrackRig::new();
    rig.settings.reminder_interval_secs = 300;
    hide(&rig.conn, rig.game, true);
    let t0 = 1000;
    let first = rig.tick(t0, 0);
    assert!(TrackRig::updated(&first, rig.game).hidden);
    let events = rig.tick(t0 + 300, 0);
    assert!(TrackRig::updated(&events, rig.game).hidden);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, TrackerEvent::Reminder(_)))
    );

    // Control: the same setup with a visible game does remind.
    let mut visible = TrackRig::new();
    visible.settings.reminder_interval_secs = 300;
    visible.tick(t0, 0);
    let events = visible.tick(t0 + 300, 0);
    assert!(!TrackRig::updated(&events, visible.game).hidden);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, TrackerEvent::Reminder(_)))
    );
}

fn steam(id: &str, name: &str) -> DiscoveredGame {
    DiscoveredGame {
        source: ImportSource::Steam,
        source_id: id.into(),
        name: name.into(),
        install_dir: None,
        executables: vec![],
        playtime_minutes: None,
        already_imported: false,
        game_source: None,
    }
}

fn hidden_of(conn: &Connection, source_id: &str) -> bool {
    conn.query_row(
        "SELECT hidden FROM games WHERE source = 'steam' AND source_id = ?1",
        [source_id],
        |r| r.get(0),
    )
    .unwrap()
}

#[test]
fn steam_import_hides_known_tools_on_first_import_only() {
    let conn = test_db();
    let games = [
        steam("431960", "Wallpaper Engine"),
        steam("1245620", "ELDEN RING"),
    ];
    let no_covers = HashMap::new();
    store::import_games(&conn, ImportSource::Steam, &games, &no_covers, vec![], 100).unwrap();
    assert!(hidden_of(&conn, "431960"));
    assert!(!hidden_of(&conn, "1245620"));

    // The user unhides it; a re-import keeps that.
    let id: i32 = conn
        .query_row("SELECT id FROM games WHERE source_id = '431960'", [], |r| {
            r.get(0)
        })
        .unwrap();
    hide(&conn, id, false);
    store::import_games(&conn, ImportSource::Steam, &games, &no_covers, vec![], 200).unwrap();
    assert!(!hidden_of(&conn, "431960"));
}

#[test]
fn migration_4_marks_existing_known_tools() {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations::migrations().to_version(&mut conn, 3).unwrap();
    for (source, source_id) in [
        ("steam", "431960"),
        ("steam", "1245620"),
        // Same id from another source must stay visible.
        ("epic", "431960"),
    ] {
        conn.execute(
            "INSERT INTO games (name, sort_name, source, source_id, created_at) VALUES ('x', 'x', ?1, ?2, 0)",
            [source, source_id],
        )
        .unwrap();
    }
    migrations::run(&mut conn).unwrap();
    assert!(hidden_of(&conn, "431960"));
    assert!(!hidden_of(&conn, "1245620"));
    let epic: bool = conn
        .query_row("SELECT hidden FROM games WHERE source = 'epic'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(!epic);
}

#[test]
fn known_tools_include_the_required_apps() {
    let ids: Vec<u32> = KNOWN_TOOLS.iter().map(|(id, _)| *id).collect();
    for required in [431960, 993090, 250820, 365670, 431730] {
        assert!(ids.contains(&required), "{required} missing");
    }
}
