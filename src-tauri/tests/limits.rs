mod common;

use chrono::NaiveDate;
use common::{BAR_EXE, FOO_EXE, TrackRig, add_game};
use gamelapse_lib::core::db::{repo, stats};
use gamelapse_lib::core::limits;
use gamelapse_lib::core::tracker::TrackerEvent;
use gamelapse_lib::core::types::{
    LimitAction, LimitInput, LimitScope, MatchMode, Reminder, SessionSource, Settings,
};

/// Local midnight of a fixed date away from any DST change.
fn midnight(day: u32) -> i64 {
    stats::local_midnight(NaiveDate::from_ymd_opt(2026, 7, day).unwrap())
}

fn input(scope: LimitScope, game_id: Option<i32>, secs: i64, action: LimitAction) -> LimitInput {
    LimitInput {
        scope,
        game_id,
        daily_secs: secs,
        action,
    }
}

fn limit_ids(events: &[TrackerEvent]) -> Vec<i32> {
    events
        .iter()
        .filter_map(|e| match e {
            TrackerEvent::LimitReached(s) => Some(s.limit.id),
            _ => None,
        })
        .collect()
}

fn reminders(events: &[TrackerEvent]) -> Vec<Reminder> {
    events
        .iter()
        .filter_map(|e| match e {
            TrackerEvent::Reminder(r) => Some(r.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn limit_validation_and_uniqueness() {
    let conn = common::test_db();
    let a = add_game(&conn, "Alpha", FOO_EXE, MatchMode::ExactPath);
    let b = add_game(&conn, "bravo", BAR_EXE, MatchMode::ExactPath);
    let n = LimitAction::Notify;

    // Ranges.
    assert!(limits::create_limit(&conn, &input(LimitScope::Global, None, 59, n)).is_err());
    assert!(limits::create_limit(&conn, &input(LimitScope::Global, None, 86_401, n)).is_err());
    // Scope and game id must agree.
    assert!(limits::create_limit(&conn, &input(LimitScope::Global, Some(a), 3600, n)).is_err());
    assert!(limits::create_limit(&conn, &input(LimitScope::Game, None, 3600, n)).is_err());
    assert!(limits::create_limit(&conn, &input(LimitScope::Game, Some(999), 3600, n)).is_err());
    assert!(limits::list_limits(&conn).unwrap().is_empty());

    let ga = limits::create_limit(&conn, &input(LimitScope::Game, Some(b), 60, n)).unwrap();
    assert_eq!(ga.game_name.as_deref(), Some("bravo"));
    let gb = limits::create_limit(&conn, &input(LimitScope::Game, Some(a), 86_400, n)).unwrap();
    let global = limits::create_limit(
        &conn,
        &input(LimitScope::Global, None, 7200, LimitAction::NotifyRepeat),
    )
    .unwrap();
    assert_eq!(global.game_id, None);
    assert_eq!(global.game_name, None);

    // One global, one per game.
    assert!(limits::create_limit(&conn, &input(LimitScope::Global, None, 600, n)).is_err());
    assert!(limits::create_limit(&conn, &input(LimitScope::Game, Some(a), 600, n)).is_err());

    // Global first, then games by name (case-insensitive).
    let order: Vec<i32> = limits::list_limits(&conn)
        .unwrap()
        .iter()
        .map(|l| l.id)
        .collect();
    assert_eq!(order, vec![global.id, gb.id, ga.id]);

    // Updates may keep their own slot but not take another's.
    let kept = limits::update_limit(
        &conn,
        gb.id,
        &input(LimitScope::Game, Some(a), 120, LimitAction::NotifyRepeat),
    )
    .unwrap();
    assert_eq!(
        (kept.daily_secs, kept.action),
        (120, LimitAction::NotifyRepeat)
    );
    assert!(limits::update_limit(&conn, gb.id, &input(LimitScope::Game, Some(b), 120, n)).is_err());
    assert!(limits::update_limit(&conn, gb.id, &input(LimitScope::Global, None, 120, n)).is_err());
    assert!(limits::update_limit(&conn, 999, &input(LimitScope::Game, Some(a), 120, n)).is_err());
    let moved =
        limits::update_limit(&conn, ga.id, &input(LimitScope::Game, Some(b), 61, n)).unwrap();
    assert_eq!(moved.daily_secs, 61);

    limits::delete_limit(&conn, global.id).unwrap();
    assert!(limits::delete_limit(&conn, global.id).is_err());
    assert!(limits::create_limit(&conn, &input(LimitScope::Global, None, 600, n)).is_ok());
}

#[test]
fn limit_status_counts_today_without_afk_and_includes_active_sessions() {
    let conn = common::test_db();
    let game = add_game(&conn, "Foo", FOO_EXE, MatchMode::ExactPath);
    let m = midnight(10);
    let now = m + 10 * 3600;
    let l = limits::create_limit(
        &conn,
        &input(LimitScope::Game, Some(game), 3600, LimitAction::Notify),
    )
    .unwrap();

    // Yesterday does not count; today's ended session has 600 s AFK.
    repo::insert_session(
        &conn,
        game,
        m - 7200,
        Some(m - 3600),
        SessionSource::Manual,
        None,
    )
    .unwrap();
    let s = repo::insert_session(
        &conn,
        game,
        m + 8 * 3600,
        Some(m + 9 * 3600),
        SessionSource::Manual,
        None,
    )
    .unwrap();
    repo::write_afk_secs(&conn, s.id, 600).unwrap();
    let status = &limits::list_statuses(&conn, now).unwrap()[0];
    assert_eq!(status.limit.id, l.id);
    assert_eq!(status.used_today_secs, 3000);
    assert_eq!(status.remaining_secs, 600);
    assert!(!status.reached);

    // An active session counts up to now.
    repo::insert_session(&conn, game, now - 900, None, SessionSource::Auto, None).unwrap();
    let status = &limits::list_statuses(&conn, now).unwrap()[0];
    assert_eq!(status.used_today_secs, 3900);
    assert_eq!(status.remaining_secs, 0);
    assert!(status.reached);
}

#[test]
fn limit_reached_fires_once_and_repeats_every_15_minutes_for_notify_repeat_only() {
    let mut rig = TrackRig::new();
    let bar = add_game(&rig.conn, "Bar", BAR_EXE, MatchMode::ExactPath);
    rig.source.set(&[FOO_EXE, BAR_EXE], None);
    let once = limits::create_limit(
        &rig.conn,
        &input(LimitScope::Game, Some(rig.game), 600, LimitAction::Notify),
    )
    .unwrap();
    let repeat = limits::create_limit(
        &rig.conn,
        &input(LimitScope::Game, Some(bar), 600, LimitAction::NotifyRepeat),
    )
    .unwrap();

    let t0 = midnight(10) + 10 * 3600;
    assert!(limit_ids(&rig.tick(t0, 0)).is_empty());
    assert!(limit_ids(&rig.tick(t0 + 599, 0)).is_empty());

    let events = rig.tick(t0 + 600, 0);
    let mut ids = limit_ids(&events);
    ids.sort_unstable();
    assert_eq!(ids, vec![once.id, repeat.id]);
    let TrackerEvent::LimitReached(status) = events
        .iter()
        .find(|e| matches!(e, TrackerEvent::LimitReached(s) if s.limit.id == once.id))
        .unwrap()
    else {
        unreachable!()
    };
    assert!(status.reached);
    assert_eq!((status.used_today_secs, status.remaining_secs), (600, 0));

    assert!(limit_ids(&rig.tick(t0 + 605, 0)).is_empty());
    assert!(limit_ids(&rig.tick(t0 + 600 + 899, 0)).is_empty());
    assert_eq!(limit_ids(&rig.tick(t0 + 600 + 900, 0)), vec![repeat.id]);
    assert!(limit_ids(&rig.tick(t0 + 600 + 901, 0)).is_empty());
    assert_eq!(limit_ids(&rig.tick(t0 + 600 + 1800, 0)), vec![repeat.id]);
}

#[test]
fn a_global_limit_counts_all_games_and_afk_time_does_not_count() {
    let mut rig = TrackRig::new();
    let bar = add_game(&rig.conn, "Bar", BAR_EXE, MatchMode::ExactPath);
    rig.source.set(&[FOO_EXE, BAR_EXE], None);
    let global = limits::create_limit(
        &rig.conn,
        &input(LimitScope::Global, None, 1200, LimitAction::Notify),
    )
    .unwrap();
    let _ = bar;

    let t0 = midnight(10) + 10 * 3600;
    rig.tick(t0, 0);
    // 600 s of two parallel sessions are 1200 s against the global limit.
    assert_eq!(limit_ids(&rig.tick(t0 + 600, 0)), vec![global.id]);

    // Both sessions AFK from here on: no repeat and no growth.
    let mut rig = TrackRig::new();
    let global = limits::create_limit(
        &rig.conn,
        &input(LimitScope::Global, None, 600, LimitAction::NotifyRepeat),
    )
    .unwrap();
    rig.tick(t0, 0);
    // Idle since t0: AFK back-dated to t0, so the 700 s at the keyboard-less desk never count.
    assert!(limit_ids(&rig.tick(t0 + 700, 700)).is_empty());
    assert!(limit_ids(&rig.tick(t0 + 5000, 5000)).is_empty());
    let status = &limits::list_statuses(&rig.conn, t0 + 5000).unwrap()[0];
    assert_eq!(status.limit.id, global.id);
    assert_eq!(status.used_today_secs, 0);
}

#[test]
fn a_limit_exceeded_earlier_does_not_fire_with_nothing_running() {
    let mut rig = TrackRig::new();
    let m = midnight(10);
    repo::insert_session(
        &rig.conn,
        rig.game,
        m + 3600,
        Some(m + 3 * 3600),
        SessionSource::Manual,
        None,
    )
    .unwrap();
    let l = limits::create_limit(
        &rig.conn,
        &input(LimitScope::Game, Some(rig.game), 600, LimitAction::Notify),
    )
    .unwrap();
    rig.source.set(&[], None);
    assert!(limit_ids(&rig.tick(m + 5 * 3600, 0)).is_empty());

    // Playing again today fires it, once.
    rig.source.set(&[FOO_EXE], None);
    assert_eq!(limit_ids(&rig.tick(m + 6 * 3600, 0)), vec![l.id]);
    assert!(limit_ids(&rig.tick(m + 6 * 3600 + 5, 0)).is_empty());
}

#[test]
fn notified_state_resets_at_local_midnight() {
    let mut rig = TrackRig::new();
    let l = limits::create_limit(
        &rig.conn,
        &input(LimitScope::Game, Some(rig.game), 600, LimitAction::Notify),
    )
    .unwrap();

    let d1 = midnight(10) + 10 * 3600;
    rig.tick(d1, 0);
    assert_eq!(limit_ids(&rig.tick(d1 + 600, 0)), vec![l.id]);
    assert!(limit_ids(&rig.tick(d1 + 1200, 0)).is_empty());

    // The game closes, and the next day it is played again past the limit.
    rig.source.set(&[], None);
    rig.tick(d1 + 1205, 0);
    rig.tick(d1 + 1210, 0);
    rig.source.set(&[FOO_EXE], None);
    let d2 = midnight(11) + 10 * 3600;
    assert!(limit_ids(&rig.tick(d2, 0)).is_empty());
    assert!(limit_ids(&rig.tick(d2 + 599, 0)).is_empty());
    assert_eq!(limit_ids(&rig.tick(d2 + 600, 0)), vec![l.id]);
}

#[test]
fn raising_a_reached_limit_lets_it_fire_again_when_reached_again() {
    let mut rig = TrackRig::new();
    let l = limits::create_limit(
        &rig.conn,
        &input(LimitScope::Game, Some(rig.game), 600, LimitAction::Notify),
    )
    .unwrap();
    let t0 = midnight(10) + 10 * 3600;
    rig.tick(t0, 0);
    assert_eq!(limit_ids(&rig.tick(t0 + 600, 0)), vec![l.id]);

    limits::update_limit(
        &rig.conn,
        l.id,
        &input(LimitScope::Game, Some(rig.game), 1200, LimitAction::Notify),
    )
    .unwrap();
    assert!(limit_ids(&rig.tick(t0 + 605, 0)).is_empty());
    assert_eq!(limit_ids(&rig.tick(t0 + 1200, 0)), vec![l.id]);
}

#[test]
fn reminders_fire_once_per_multiple_of_the_interval() {
    let mut rig = TrackRig::new();
    rig.settings.reminder_interval_secs = 300;
    let t0 = 10_000;
    assert!(reminders(&rig.tick(t0, 0)).is_empty());
    assert!(reminders(&rig.tick(t0 + 299, 0)).is_empty());

    let r = reminders(&rig.tick(t0 + 300, 0));
    assert_eq!(r.len(), 1);
    assert_eq!((r[0].game_id, r[0].elapsed_secs), (rig.game, 300));
    assert_eq!(r[0].game_name, "Foo");
    assert!(reminders(&rig.tick(t0 + 301, 0)).is_empty());
    assert_eq!(reminders(&rig.tick(t0 + 600, 0)).len(), 1);

    // A long gap between polls crosses several multiples but reminds once.
    let r = reminders(&rig.tick(t0 + 1500, 0));
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].elapsed_secs, 1500);
    assert!(reminders(&rig.tick(t0 + 1501, 0)).is_empty());
}

#[test]
fn reminders_use_non_afk_time_and_can_be_off() {
    let mut rig = TrackRig::new();
    rig.settings.reminder_interval_secs = 300;
    let t0 = 10_000;
    rig.tick(t0, 0);
    // Idle from t0: elapsed stays at 0 however long it lasts.
    assert!(reminders(&rig.tick(t0 + 601, 601)).is_empty());
    assert!(reminders(&rig.tick(t0 + 2000, 2000)).is_empty());
    // Back at the keyboard: elapsed resumes from 0.
    assert!(reminders(&rig.tick(t0 + 2005, 0)).is_empty());
    assert_eq!(reminders(&rig.tick(t0 + 2305, 300)).len(), 1);

    let mut off = TrackRig::new();
    assert_eq!(off.settings.reminder_interval_secs, 0);
    off.tick(t0, 0);
    assert!(reminders(&off.tick(t0 + 5000, 0)).is_empty());
}

#[test]
fn a_reopened_session_does_not_replay_past_reminders() {
    let mut rig = TrackRig::new();
    rig.settings.reminder_interval_secs = 300;
    rig.tick(1000, 0);
    assert_eq!(reminders(&rig.tick(1300, 0)).len(), 1);
    rig.tick(1400, 0);

    rig.source.set(&[], None);
    rig.tick(1405, 0);
    rig.tick(1410, 0); // ended at 1400
    rig.source.set(&[FOO_EXE], None);
    // Reopened within the merge gap with 450 s elapsed: multiple 1 was already reminded.
    assert!(reminders(&rig.tick(1450, 0)).is_empty());
    assert_eq!(reminders(&rig.tick(1600, 0)).len(), 1);
}

#[test]
fn reminder_interval_validation() {
    let ok = |secs| {
        Settings::default()
            .with_update(&gamelapse_lib::core::types::SettingsUpdate {
                reminder_interval_secs: Some(secs),
                ..Default::default()
            })
            .is_ok()
    };
    assert!(ok(0));
    assert!(ok(300));
    assert!(ok(14_400));
    assert!(!ok(1));
    assert!(!ok(299));
    assert!(!ok(14_401));
    let defaults = Settings::default();
    assert!(defaults.notifications_enabled);
    assert_eq!(defaults.reminder_interval_secs, 0);
}
