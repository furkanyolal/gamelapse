mod common;

use common::{add_game, test_db};
use gamelapse_lib::core::db::{library, repo, stats};
use gamelapse_lib::core::types::{MatchMode, SessionSource};
use rusqlite::params;

fn tag_names(tags: &[gamelapse_lib::core::types::Tag]) -> Vec<&str> {
    tags.iter().map(|t| t.name.as_str()).collect()
}

fn session(conn: &rusqlite::Connection, game: i32, start: i64, end: Option<i64>) {
    repo::insert_session(conn, game, start, end, SessionSource::Manual, None).unwrap();
}

fn count(conn: &rusqlite::Connection, sql: &str, game: i32) -> i64 {
    conn.query_row(sql, [game], |r| r.get(0)).unwrap()
}

#[test]
fn tag_names_are_trimmed_and_validated() {
    let conn = test_db();
    let tag = library::create_tag(&conn, "  RPG  ").unwrap();
    assert_eq!(tag.name, "RPG");

    assert!(library::create_tag(&conn, "   ").is_err());
    assert!(library::create_tag(&conn, &"x".repeat(41)).is_err());
    assert!(library::create_tag(&conn, &"x".repeat(40)).is_ok());
    assert!(library::rename_tag(&conn, tag.id, "").is_err());
    assert!(library::rename_tag(&conn, 999, "Other").is_err());
}

#[test]
fn tag_names_are_unique_ignoring_case() {
    let conn = test_db();
    let rpg = library::create_tag(&conn, "RPG").unwrap();
    let co_op = library::create_tag(&conn, "Co-op").unwrap();
    assert!(library::create_tag(&conn, "rpg").is_err());
    assert!(library::create_tag(&conn, " Rpg ").is_err());
    assert!(library::rename_tag(&conn, co_op.id, "RPG").is_err());
    // Changing only the case of its own name is fine.
    assert_eq!(
        library::rename_tag(&conn, rpg.id, "rpg").unwrap().name,
        "rpg"
    );
    let names: Vec<String> = library::list_tags(&conn)
        .unwrap()
        .into_iter()
        .map(|t| t.name)
        .collect();
    assert_eq!(names, ["Co-op", "rpg"]);
}

#[test]
fn set_game_tags_replaces_and_delete_cascades() {
    let conn = test_db();
    let game = add_game(&conn, "Foo", r"c:\foo.exe", MatchMode::ExactPath);
    let a = library::create_tag(&conn, "Alpha").unwrap();
    let b = library::create_tag(&conn, "Beta").unwrap();
    let c = library::create_tag(&conn, "Gamma").unwrap();

    let set = library::set_game_tags(&conn, game, &[c.id, a.id, a.id]).unwrap();
    assert_eq!(tag_names(&set), ["Alpha", "Gamma"]);
    let set = library::set_game_tags(&conn, game, &[b.id]).unwrap();
    assert_eq!(tag_names(&set), ["Beta"]);
    assert_eq!(
        tag_names(&library::tags_for_game(&conn, game).unwrap()),
        ["Beta"]
    );

    assert!(library::set_game_tags(&conn, game, &[b.id, 999]).is_err());
    assert!(library::set_game_tags(&conn, 999, &[b.id]).is_err());
    // A failed call leaves the previous tags in place.
    assert_eq!(
        tag_names(&library::tags_for_game(&conn, game).unwrap()),
        ["Beta"]
    );

    library::delete_tag(&conn, b.id).unwrap();
    assert!(library::tags_for_game(&conn, game).unwrap().is_empty());
    assert!(library::set_game_tags(&conn, game, &[]).unwrap().is_empty());
}

#[test]
fn game_summaries_carry_sorted_tags() {
    let conn = test_db();
    let tagged = add_game(&conn, "Aaa", r"c:\a.exe", MatchMode::ExactPath);
    let plain = add_game(&conn, "Bbb", r"c:\b.exe", MatchMode::ExactPath);
    let zed = library::create_tag(&conn, "Zed").unwrap();
    let ace = library::create_tag(&conn, "ace").unwrap();
    library::set_game_tags(&conn, tagged, &[zed.id, ace.id]).unwrap();

    let list = stats::list_game_summaries(&conn, true, 1_000).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].game.id, tagged);
    assert_eq!(tag_names(&list[0].tags), ["ace", "Zed"]);
    assert_eq!(list[1].game.id, plain);
    assert!(list[1].tags.is_empty());

    let one = stats::game_summary(&conn, repo::get_game(&conn, tagged).unwrap(), 1_000).unwrap();
    assert_eq!(tag_names(&one.tags), ["ace", "Zed"]);
}

#[test]
fn merge_moves_everything_onto_the_survivor() {
    let conn = test_db();
    let keep = add_game(&conn, "Keep", r"c:\keep\a.exe", MatchMode::ExactPath);
    // Same path as the survivor's rule: skipped. Extra unique rule on `dup`: moved.
    let dup = add_game(&conn, "Dup", r"c:\keep\a.exe", MatchMode::ExactPath);
    let other = add_game(&conn, "Other", r"c:\other\o.exe", MatchMode::ExactPath);
    repo::insert_executable(&conn, dup, r"c:\dup\d.exe", MatchMode::ExactPath).unwrap();

    session(&conn, keep, 100, Some(200));
    session(&conn, dup, 300, Some(400));
    session(&conn, dup, 500, Some(600));
    session(&conn, other, 700, Some(800));

    let t1 = library::create_tag(&conn, "One").unwrap();
    let t2 = library::create_tag(&conn, "Two").unwrap();
    library::set_game_tags(&conn, keep, &[t1.id]).unwrap();
    library::set_game_tags(&conn, dup, &[t1.id, t2.id]).unwrap();

    let put = |game: i32, source: &str, minutes: i64, at: i64| {
        conn.execute(
            "INSERT INTO imported_playtime (game_id, source, minutes, fetched_at) VALUES (?1, ?2, ?3, ?4)",
            params![game, source, minutes, at],
        )
        .unwrap();
    };
    put(keep, "steam", 100, 1);
    put(dup, "steam", 250, 2);
    put(keep, "gog", 90, 3);
    put(dup, "gog", 10, 4);
    put(dup, "epic", 5, 5);

    let result = library::merge_games(&conn, keep, &[dup, other, dup]).unwrap();
    assert_eq!(result.survivor_id, keep);
    assert_eq!(result.merged_sessions, 3);
    assert_eq!(result.merged_executables, 2); // dup's unique rule and other's rule

    assert!(repo::get_game(&conn, dup).is_err());
    assert!(repo::get_game(&conn, other).is_err());
    assert_eq!(
        count(
            &conn,
            "SELECT COUNT(*) FROM sessions WHERE game_id = ?1",
            keep
        ),
        4
    );
    let paths: Vec<String> = repo::list_executables(&conn, keep)
        .unwrap()
        .into_iter()
        .map(|e| e.exe_path)
        .collect();
    assert_eq!(
        paths,
        [r"c:\keep\a.exe", r"c:\other\o.exe", r"c:\dup\d.exe"]
    );
    assert_eq!(
        tag_names(&library::tags_for_game(&conn, keep).unwrap()),
        ["One", "Two"]
    );

    let minutes = |source: &str| -> (i64, i64) {
        conn.query_row(
            "SELECT minutes, fetched_at FROM imported_playtime WHERE game_id = ?1 AND source = ?2",
            params![keep, source],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
    };
    assert_eq!(minutes("steam"), (250, 2));
    assert_eq!(minutes("gog"), (90, 3));
    assert_eq!(minutes("epic"), (5, 5));
    assert_eq!(
        count(
            &conn,
            "SELECT COUNT(*) FROM imported_playtime WHERE game_id = ?1",
            keep
        ),
        3
    );
}

#[test]
fn merge_rejects_bad_input_and_changes_nothing() {
    let conn = test_db();
    let keep = add_game(&conn, "Keep", r"c:\k.exe", MatchMode::ExactPath);
    let busy = add_game(&conn, "Busy", r"c:\b.exe", MatchMode::ExactPath);
    let idle = add_game(&conn, "Idle", r"c:\i.exe", MatchMode::ExactPath);
    session(&conn, idle, 10, Some(20));

    assert!(library::merge_games(&conn, keep, &[keep, idle]).is_err());
    assert!(library::merge_games(&conn, keep, &[]).is_err());
    assert!(library::merge_games(&conn, keep, &[idle, 999]).is_err());
    assert!(library::merge_games(&conn, 999, &[idle]).is_err());

    session(&conn, busy, 30, None);
    assert!(library::merge_games(&conn, keep, &[idle, busy]).is_err());
    // The survivor's own active session blocks too.
    assert!(library::merge_games(&conn, busy, &[idle]).is_err());

    assert!(repo::get_game(&conn, idle).is_ok());
    assert_eq!(
        count(
            &conn,
            "SELECT COUNT(*) FROM sessions WHERE game_id = ?1",
            idle
        ),
        1
    );
    assert_eq!(
        count(
            &conn,
            "SELECT COUNT(*) FROM sessions WHERE game_id = ?1",
            keep
        ),
        0
    );
}
