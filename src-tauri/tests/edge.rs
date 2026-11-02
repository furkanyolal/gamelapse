mod common;

use common::{BAR_EXE, FOO_EXE, add_game, test_db};
use gamelapse_lib::core::db::{library, repo};
use gamelapse_lib::core::limits;
use gamelapse_lib::core::types::{
    LimitAction, LimitInput, LimitScope, MatchMode, SessionQuery, SessionSource,
};

#[test]
fn deleting_a_game_with_an_active_session_is_rejected() {
    let conn = test_db();
    let game = add_game(&conn, "Foo", FOO_EXE, MatchMode::ExactPath);
    repo::insert_session(&conn, game, 1000, None, SessionSource::Manual, None).unwrap();

    assert!(repo::delete_game(&conn, game).is_err());
    assert!(repo::get_game(&conn, game).is_ok());

    // Once the session is over the game can go, and its sessions with it.
    let open = repo::active_session_for_game(&conn, game).unwrap().unwrap();
    repo::end_session(&conn, open.id, 2000).unwrap();
    repo::delete_game(&conn, game).unwrap();
    assert!(repo::get_game(&conn, game).is_err());
    assert!(repo::get_session(&conn, open.id, 3000).is_err());
}

#[test]
fn merging_deletes_the_limits_of_merged_games_only() {
    let conn = test_db();
    let keep = add_game(&conn, "Keep", FOO_EXE, MatchMode::ExactPath);
    let gone = add_game(&conn, "Gone", BAR_EXE, MatchMode::ExactPath);
    for game in [keep, gone] {
        limits::create_limit(
            &conn,
            &LimitInput {
                scope: LimitScope::Game,
                game_id: Some(game),
                daily_secs: 3600,
                action: LimitAction::Notify,
            },
        )
        .unwrap();
    }

    library::merge_games(&conn, keep, &[gone]).unwrap();

    let remaining = limits::list_statuses(&conn, 10_000).unwrap();
    let games: Vec<Option<i32>> = remaining.iter().map(|s| s.limit.game_id).collect();
    assert_eq!(games, vec![Some(keep)]);
}

fn query(game_id: Option<i32>, limit: u32, offset: u32) -> SessionQuery {
    SessionQuery {
        game_id,
        from: None,
        to: None,
        limit: limit as _,
        offset: offset as _,
        include_hidden: false,
    }
}

#[test]
fn list_sessions_filters_by_game_and_pages_with_offsets() {
    let conn = test_db();
    let foo = add_game(&conn, "Foo", FOO_EXE, MatchMode::ExactPath);
    let bar = add_game(&conn, "Bar", BAR_EXE, MatchMode::ExactPath);
    // Foo sessions start at 100, 200, ..., 500; Bar has one at 250.
    for i in 1..=5 {
        let start = i * 100;
        repo::insert_session(
            &conn,
            foo,
            start,
            Some(start + 50),
            SessionSource::Manual,
            None,
        )
        .unwrap();
    }
    repo::insert_session(&conn, bar, 250, Some(260), SessionSource::Manual, None).unwrap();

    let starts = |q: SessionQuery| -> Vec<i64> {
        repo::list_sessions(&conn, &q, 10_000)
            .unwrap()
            .iter()
            .map(|s| s.session.started_at)
            .collect()
    };

    assert_eq!(starts(query(Some(bar), 10, 0)), vec![250]);
    assert_eq!(
        starts(query(Some(foo), 10, 0)),
        vec![500, 400, 300, 200, 100]
    );
    // Newest first across games.
    assert_eq!(
        starts(query(None, 10, 0)),
        vec![500, 400, 300, 250, 200, 100]
    );
    // Pages of two, filtered and unfiltered.
    assert_eq!(starts(query(Some(foo), 2, 0)), vec![500, 400]);
    assert_eq!(starts(query(Some(foo), 2, 2)), vec![300, 200]);
    assert_eq!(starts(query(Some(foo), 2, 4)), vec![100]);
    assert!(starts(query(Some(foo), 2, 6)).is_empty());
    assert_eq!(starts(query(None, 2, 2)), vec![300, 250]);
}
