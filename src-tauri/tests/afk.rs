mod common;

use common::TrackRig;
use gamelapse_lib::core::db::{repo, stats};
use gamelapse_lib::core::idle::IdleReading;
use gamelapse_lib::core::types::GameUpdate;

fn session_id(rig: &TrackRig) -> i32 {
    repo::active_session_for_game(&rig.conn, rig.game)
        .unwrap()
        .expect("active session")
        .id
}

#[test]
fn crossing_the_threshold_back_dates_afk_to_the_last_input() {
    let mut rig = TrackRig::new(); // threshold 600
    rig.tick(1000, 0);
    let id = session_id(&rig);

    let a = TrackRig::updated(&rig.tick(1300, 300), rig.game);
    assert!(!a.is_afk);
    assert_eq!(a.afk_secs, 0);
    let a = TrackRig::updated(&rig.tick(1600, 600), rig.game);
    assert!(!a.is_afk, "idle must exceed the threshold, not equal it");

    // The last input was at 1000, so the whole idle stretch becomes AFK at once.
    let a = TrackRig::updated(&rig.tick(1601, 601), rig.game);
    assert!(a.is_afk);
    assert_eq!(a.afk_secs, 601);
    assert_eq!(a.elapsed_secs, 0);
    assert_eq!(rig.session(id, 1601).afk_secs, 601, "persisted on the poll");

    // Then it grows per poll.
    let a = TrackRig::updated(&rig.tick(1606, 606), rig.game);
    assert_eq!((a.afk_secs, a.elapsed_secs), (606, 0));
    assert_eq!(rig.session(id, 1606).afk_secs, 606);
}

#[test]
fn afk_ends_on_the_first_poll_with_fresh_input() {
    let mut rig = TrackRig::new();
    rig.tick(1000, 0);
    rig.tick(1601, 601);
    rig.tick(1606, 606);

    let a = TrackRig::updated(&rig.tick(1611, 2), rig.game);
    assert!(!a.is_afk);
    assert_eq!(a.afk_secs, 606, "the AFK total stops growing");
    assert_eq!(a.elapsed_secs, 5);

    // Play counts again, and a later idle stretch starts a new AFK episode at its own last input.
    let a = TrackRig::updated(&rig.tick(1616, 7), rig.game);
    assert_eq!((a.is_afk, a.afk_secs, a.elapsed_secs), (false, 606, 10));
    rig.tick(1621, 0); // input at 1621
    let a = TrackRig::updated(&rig.tick(2230, 609), rig.game);
    assert!(a.is_afk);
    assert_eq!(a.afk_secs, 606 + 609);
}

#[test]
fn afk_never_reaches_back_before_the_session_started() {
    let mut rig = TrackRig::new();
    // The user was away already when the game launched.
    let a = TrackRig::updated(&rig.tick(1000, 900), rig.game);
    assert!(a.is_afk);
    assert_eq!(a.afk_secs, 0);
    let a = TrackRig::updated(&rig.tick(1005, 905), rig.game);
    assert_eq!((a.afk_secs, a.elapsed_secs), (5, 0));
}

#[test]
fn games_with_afk_detection_disabled_never_go_afk() {
    let mut rig = TrackRig::new();
    repo::update_game(
        &rig.conn,
        rig.game,
        &GameUpdate {
            name: None,
            hidden: None,
            afk_detection_enabled: Some(false),
        },
    )
    .unwrap();
    rig.tick(1000, 0);
    let a = TrackRig::updated(&rig.tick(5000, 4000), rig.game);
    assert!(!a.is_afk);
    assert_eq!(a.afk_secs, 0);
    assert_eq!(a.elapsed_secs, 4000);
}

#[test]
fn threshold_zero_turns_detection_off() {
    let mut rig = TrackRig::new();
    rig.settings.afk_threshold_secs = 0;
    rig.tick(1000, 0);
    let a = TrackRig::updated(&rig.tick(1100, 100), rig.game);
    assert!(!a.is_afk);
}

#[test]
fn manual_sessions_follow_the_same_rules() {
    let mut rig = TrackRig::new();
    rig.source.set(&[], None); // no process: only the manual session runs
    rig.tracker
        .start_manual(&rig.conn, true, rig.game, 1000)
        .unwrap();
    let id = session_id(&rig);

    let a = TrackRig::updated(&rig.tick(1601, 601), rig.game);
    assert!(a.is_afk);
    assert_eq!(a.afk_secs, 601);
    assert_eq!(rig.session(id, 1601).afk_secs, 601);

    let a = TrackRig::updated(&rig.tick(1606, 1), rig.game);
    assert!(!a.is_afk);
    assert_eq!(a.elapsed_secs, 5);
}

#[test]
fn a_locked_workstation_counts_as_idle() {
    let mut rig = TrackRig::new();
    rig.tick(1000, 0);
    // Locked 50 s after the last input: AFK from that input, without waiting for the threshold.
    let locked = IdleReading {
        idle_secs: Some(50),
        locked: true,
    };
    let a = TrackRig::updated(&rig.tick_with(1050, locked), rig.game);
    assert!(a.is_afk);
    assert_eq!(a.afk_secs, 50);

    // Without an idle reading it starts now and grows per poll.
    let mut rig = TrackRig::new();
    rig.tick(1000, 0);
    let unknown_locked = IdleReading {
        idle_secs: None,
        locked: true,
    };
    let a = TrackRig::updated(&rig.tick_with(1005, unknown_locked), rig.game);
    assert_eq!((a.is_afk, a.afk_secs), (true, 0));
    let a = TrackRig::updated(&rig.tick_with(1010, unknown_locked), rig.game);
    assert_eq!(a.afk_secs, 5);

    // Unlocking with fresh input ends it.
    let a = TrackRig::updated(&rig.tick(1015, 1), rig.game);
    assert!(!a.is_afk);
}

#[test]
fn an_unknown_idle_reading_is_treated_as_input() {
    let mut rig = TrackRig::new();
    rig.tick(1000, 0);
    rig.tick(1601, 601);
    let a = TrackRig::updated(&rig.tick_with(1606, IdleReading::default()), rig.game);
    assert!(!a.is_afk);
}

#[test]
fn duration_and_stats_exclude_afk_for_active_and_ended_sessions() {
    let mut rig = TrackRig::new();
    rig.tick(1000, 0);
    rig.tick(1601, 601); // afk 601
    rig.tick(1606, 606); // afk 606
    rig.tick(1611, 0); // back
    rig.tick(1711, 100); // 100 s of play
    let id = session_id(&rig);
    let now = 1711;

    // Active: wall 711, afk 606, counted 105.
    let s = rig.session(id, now);
    assert_eq!((s.afk_secs, s.duration_secs), (606, 105));
    let active = TrackRig::updated(&rig.tick(1711, 100), rig.game);
    assert_eq!(active.elapsed_secs, 105);
    assert_eq!(
        stats::range_secs(&rig.conn, 0, 10_000, now, None).unwrap(),
        105
    );
    assert_eq!(
        stats::range_secs(&rig.conn, 0, 10_000, now, Some(rig.game)).unwrap(),
        105
    );
    let totals = stats::game_totals(&rig.conn, now, None, None, 5).unwrap();
    assert_eq!(totals[0].total_secs, 105);
    let game = repo::get_game(&rig.conn, rig.game).unwrap();
    assert_eq!(
        stats::game_summary(&rig.conn, game, now)
            .unwrap()
            .total_secs,
        105
    );
    let listed = repo::list_sessions(
        &rig.conn,
        &gamelapse_lib::core::types::SessionQuery {
            game_id: None,
            from: None,
            to: None,
            limit: 10,
            offset: 0,
        },
        now,
    )
    .unwrap();
    assert_eq!(listed[0].session.duration_secs, 105);

    // Ended: same numbers once closed.
    let (ended, _) = rig.tracker.stop_session(&rig.conn, true, id, 1711).unwrap();
    assert_eq!((ended.afk_secs, ended.duration_secs), (606, 105));
    assert_eq!(
        stats::range_secs(&rig.conn, 0, 10_000, 5000, None).unwrap(),
        105
    );
    let totals = stats::game_totals(&rig.conn, 5000, None, None, 5).unwrap();
    assert_eq!(totals[0].total_secs, 105);
    let dashboard = stats::dashboard(&rig.conn, 5000).unwrap();
    assert_eq!(dashboard.all_time_secs, 105);
    assert_eq!(
        stats::daily_totals(&rig.conn, 1711, 1, None).unwrap()[0].total_secs,
        105
    );
}
