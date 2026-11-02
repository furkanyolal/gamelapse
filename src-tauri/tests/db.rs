use gametimetracker_lib::core::db::migrations;
use rusqlite::Connection;

#[test]
fn migrations_are_valid() {
    migrations::migrations().validate().unwrap();
}

#[test]
fn creates_schema_and_sets_user_version() {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations::run(&mut conn).unwrap();

    let games: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='games'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(games, 1);

    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, migrations::count() as i64);
}

#[test]
fn foreign_keys_cascade_from_games() {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    migrations::run(&mut conn).unwrap();
    conn.execute(
        "INSERT INTO games (id, name, sort_name, created_at) VALUES (1, 'A', 'a', 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO sessions (game_id, started_at) VALUES (1, 0)",
        [],
    )
    .unwrap();
    conn.execute("DELETE FROM games WHERE id = 1", []).unwrap();
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 0);
}
