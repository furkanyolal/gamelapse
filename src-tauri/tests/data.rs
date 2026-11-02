mod common;

use std::path::Path;

use chrono::{Local, SecondsFormat, TimeZone};
use common::{add_game, test_db};
use gametimetracker_lib::core::db::{self, migrations, repo};
use gametimetracker_lib::core::types::{ExportFormat, ExportOptions, MatchMode, SessionSource};
use gametimetracker_lib::core::{custom_covers, export};
use rusqlite::Connection;
use serde_json::Value;

const NOW: i64 = 1_800_000_000;

fn iso(ts: i64) -> String {
    Local
        .timestamp_opt(ts, 0)
        .earliest()
        .unwrap()
        .to_rfc3339_opts(SecondsFormat::Secs, false)
}

fn options(format: ExportFormat) -> ExportOptions {
    ExportOptions {
        format,
        from: None,
        to: None,
        game_id: None,
    }
}

/// Two games; "Foo, the \"Game\"" has two sessions (one with a nasty note), "Bar" one
/// that is still running, "Idle" none.
fn seeded() -> Connection {
    let conn = test_db();
    let alpha = add_game(
        &conn,
        "Foo, the \"Game\"",
        r"c:\alpha.exe",
        MatchMode::ExactPath,
    );
    let bar = add_game(&conn, "Bar", r"c:\bar.exe", MatchMode::ExactPath);
    add_game(&conn, "Idle", r"c:\idle.exe", MatchMode::ExactPath);
    repo::insert_session(&conn, alpha, 1_000, Some(1_600), SessionSource::Auto, None).unwrap();
    repo::insert_session(
        &conn,
        alpha,
        2_000,
        Some(2_100),
        SessionSource::Manual,
        Some("first, \"best\"\nrun"),
    )
    .unwrap();
    repo::insert_session(&conn, bar, 3_000, None, SessionSource::Auto, None).unwrap();
    conn
}

#[test]
fn csv_export_quotes_and_formats_rows() {
    let conn = seeded();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.csv");
    let result = export::export_to_path(&conn, &options(ExportFormat::Csv), &path, 3_500).unwrap();
    assert_eq!((result.sessions, result.games), (3, 2));
    assert_eq!(result.path, path.display().to_string());

    let text = std::fs::read_to_string(&path).unwrap();
    let expected = format!(
        "session_id,game,started_at,ended_at,duration_secs,afk_secs,source,note\r\n\
         1,\"Foo, the \"\"Game\"\"\",{},{},600,0,auto,\r\n\
         2,\"Foo, the \"\"Game\"\"\",{},{},100,0,manual,\"first, \"\"best\"\"\nrun\"\r\n\
         3,Bar,{},,500,0,auto,\r\n",
        iso(1_000),
        iso(1_600),
        iso(2_000),
        iso(2_100),
        iso(3_000),
    );
    assert_eq!(text, expected);
}

#[test]
fn export_filters_by_game_and_period() {
    let conn = seeded();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.csv");

    let mut opts = options(ExportFormat::Csv);
    opts.from = Some(1_500);
    opts.to = Some(3_000);
    let result = export::export_to_path(&conn, &opts, &path, NOW).unwrap();
    assert_eq!((result.sessions, result.games), (1, 1));

    let mut opts = options(ExportFormat::Csv);
    opts.game_id = Some(3);
    let result = export::export_to_path(&conn, &opts, &path, NOW).unwrap();
    assert_eq!((result.sessions, result.games), (0, 0));
    assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 1);
}

#[test]
fn json_export_has_games_and_sessions() {
    let conn = seeded();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.json");
    let result = export::export_to_path(&conn, &options(ExportFormat::Json), &path, NOW).unwrap();
    assert_eq!((result.sessions, result.games), (3, 2));

    let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(doc["exportedAt"], NOW);
    let games = doc["games"].as_array().unwrap();
    let names: Vec<&str> = games.iter().map(|g| g["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Bar", "Foo, the \"Game\""]); // Idle has no sessions
    assert_eq!(games[0]["sortName"], "bar");

    let sessions = doc["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 3);
    assert_eq!(sessions[0]["id"], 1);
    assert_eq!(sessions[0]["gameName"], "Foo, the \"Game\"");
    assert_eq!(sessions[0]["startedAt"], 1_000);
    assert_eq!(sessions[0]["durationSecs"], 600);
    assert_eq!(sessions[1]["note"], "first, \"best\"\nrun");
    assert!(sessions[2]["endedAt"].is_null());
    assert_eq!(sessions[2]["durationSecs"], NOW - 3_000);
}

#[test]
fn default_export_names_follow_the_contract() {
    let name = export::default_file_name(ExportFormat::Csv, NOW);
    let day = Local.timestamp_opt(NOW, 0).unwrap().format("%Y%m%d");
    assert_eq!(name, format!("gametimetracker-sessions-{day}.csv"));
    assert!(export::default_file_name(ExportFormat::Json, NOW).ends_with(".json"));
}

fn user_version(conn: &Connection) -> i64 {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap()
}

fn session_count(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
        .unwrap()
}

/// A database made by the first release: schema version 1 with two sessions.
fn make_old_backup(path: &Path) {
    let mut conn = Connection::open(path).unwrap();
    migrations::migrations().to_version(&mut conn, 1).unwrap();
    assert_eq!(user_version(&conn), 1);
    conn.execute(
        "INSERT INTO games (id, name, sort_name, created_at) VALUES (1, 'Old', 'old', 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO sessions (game_id, started_at, ended_at) VALUES (1, 10, 20), (1, 30, 40)",
        [],
    )
    .unwrap();
}

#[test]
fn backup_then_restore_round_trips_and_migrates() {
    assert!(migrations::count() > 1, "needs a migration to bump");
    let dir = tempfile::tempdir().unwrap();
    let live_path = dir.path().join("gametimetracker.db");
    let old = dir.path().join("old.db");
    make_old_backup(&old);

    let mut live = db::open(&live_path).unwrap();
    let game = add_game(&live, "Current", r"c:\c.exe", MatchMode::ExactPath);
    repo::insert_session(&live, game, 1, Some(2), SessionSource::Manual, None).unwrap();
    assert_eq!(session_count(&live), 1);

    // Backup: a consistent copy, overwriting an existing file.
    let backup = dir.path().join("backup.db");
    std::fs::write(&backup, b"stale").unwrap();
    db::backup_to(&live, &backup).unwrap();
    db::validate_backup(&backup).unwrap();
    {
        let copy = Connection::open(&backup).unwrap();
        assert_eq!(session_count(&copy), 1);
        assert_eq!(user_version(&copy), migrations::count() as i64);
    }

    // Restore the old, version-1 file over the live database.
    db::replace_database(&mut live, &live_path, &old).unwrap();
    assert_eq!(session_count(&live), 2);
    assert_eq!(user_version(&live), migrations::count() as i64);
    let tables: i64 = live
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'import_sources'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(tables, 1);
    let on: i64 = live
        .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
        .unwrap();
    assert_eq!(on, 1);

    // The previous database was kept.
    let bak = dir.path().join("gametimetracker.db.bak");
    assert!(bak.exists());
    assert_eq!(session_count(&Connection::open(&bak).unwrap()), 1);

    // And the backup restores the original data.
    db::replace_database(&mut live, &live_path, &backup).unwrap();
    assert_eq!(session_count(&live), 1);
    assert_eq!(repo::list_games(&live, true).unwrap()[0].name, "Current");
}

#[test]
fn restore_rejects_invalid_files_and_keeps_the_live_database() {
    let dir = tempfile::tempdir().unwrap();
    let live_path = dir.path().join("gametimetracker.db");
    let mut live = db::open(&live_path).unwrap();
    let game = add_game(&live, "Current", r"c:\c.exe", MatchMode::ExactPath);
    repo::insert_session(&live, game, 1, Some(2), SessionSource::Manual, None).unwrap();

    let text = dir.path().join("notes.db");
    std::fs::write(&text, b"definitely not sqlite").unwrap();
    let empty = dir.path().join("empty.db");
    Connection::open(&empty)
        .unwrap()
        .execute("CREATE TABLE other (x)", [])
        .unwrap();
    let newer = dir.path().join("newer.db");
    Connection::open(&newer)
        .unwrap()
        .execute_batch("CREATE TABLE games (id); PRAGMA user_version = 999;")
        .unwrap();

    for bad in [&text, &empty, &newer, &dir.path().join("missing.db")] {
        assert!(db::replace_database(&mut live, &live_path, bad).is_err());
        assert_eq!(session_count(&live), 1);
    }
    assert!(!dir.path().join("gametimetracker.db.bak").exists());
}

#[test]
fn custom_covers_are_copied_replaced_and_removed() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    let png = src.join("a.PNG");
    let jpg = src.join("b.jpg");
    std::fs::write(&png, b"png-bytes").unwrap();
    std::fs::write(&jpg, b"jpg-bytes").unwrap();

    let dest = custom_covers::set(&data, 7, &png).unwrap();
    assert_eq!(dest, data.join("covers").join("custom").join("7.png"));
    assert_eq!(std::fs::read(&dest).unwrap(), b"png-bytes");

    // Another extension replaces the old file; other games' covers stay.
    let other = custom_covers::set(&data, 70, &png).unwrap();
    let dest = custom_covers::set(&data, 7, &jpg).unwrap();
    assert_eq!(dest.file_name().unwrap(), "7.jpg");
    assert!(!data.join("covers/custom/7.png").exists());
    assert!(other.exists());

    // Choosing the stored file again is harmless.
    custom_covers::set(&data, 7, &dest).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), b"jpg-bytes");

    assert!(custom_covers::set(&data, 7, &src.join("notes.txt")).is_err());
    assert!(custom_covers::set(&data, 7, &src.join("missing.png")).is_err());

    custom_covers::remove(&data, 7);
    assert!(!dest.exists());
    assert!(other.exists());
}
