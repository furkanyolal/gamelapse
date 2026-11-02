mod common;

use common::{FakeClock, FakeSource, add_game, test_db};
use gametimetracker_lib::core::db::repo;
use gametimetracker_lib::core::tracker::{Tracker, TrackerEvent, matches};
use gametimetracker_lib::core::types::{
    GameExecutable, MatchMode, SessionQuery, SessionSource, Settings,
};

const GAME_EXE: &str = r"c:\games\foo\foo.exe";

fn exe(path: &str, mode: MatchMode) -> GameExecutable {
    GameExecutable {
        id: 1,
        game_id: 1,
        exe_path: path.to_string(),
        match_mode: mode,
    }
}

fn all_sessions(
    conn: &rusqlite::Connection,
) -> Vec<gametimetracker_lib::core::types::SessionWithGame> {
    repo::list_sessions(
        conn,
        &SessionQuery {
            game_id: None,
            from: None,
            to: None,
            limit: 100,
            offset: 0,
        },
        1_000_000,
    )
    .unwrap()
}

#[test]
fn matcher_exact_path() {
    let e = exe(GAME_EXE, MatchMode::ExactPath);
    assert!(matches(&e, GAME_EXE));
    assert!(!matches(&e, r"c:\games\foo\other.exe"));
    assert!(!matches(&e, r"d:\games\foo\foo.exe"));
}

#[test]
fn matcher_install_dir_prefix() {
    let e = exe(r"c:\games\foo", MatchMode::InstallDirPrefix);
    assert!(matches(&e, r"c:\games\foo\bin\game.exe"));
    assert!(matches(&e, GAME_EXE));
    // A sibling directory sharing the prefix must not match.
    assert!(!matches(&e, r"c:\games\foobar\game.exe"));
    assert!(!matches(&e, r"c:\games\foo"));
}

#[test]
fn matcher_exe_name() {
    let e = exe("foo.exe", MatchMode::ExeName);
    assert!(matches(&e, GAME_EXE));
    assert!(matches(&e, r"d:\elsewhere\foo.exe"));
    assert!(!matches(&e, r"c:\games\barfoo.exe"));
    assert!(!matches(&e, r"c:\games\foo.exe\bar.exe"));
}

struct Rig {
    conn: rusqlite::Connection,
    tracker: Tracker,
    source: FakeSource,
    clock: FakeClock,
    settings: Settings,
    game: i32,
}

impl Rig {
    fn new() -> Self {
        let conn = test_db();
        let game = add_game(&conn, "Foo", GAME_EXE, MatchMode::ExactPath);
        Self {
            conn,
            tracker: Tracker::new(),
            source: FakeSource::default(),
            clock: FakeClock::new(1000),
            settings: Settings::default(),
            game,
        }
    }

    fn tick(&mut self, t: i64) -> Vec<TrackerEvent> {
        use gametimetracker_lib::core::tracker::ProcessSource;
        self.clock.set(t);
        let mut source = self.source.clone();
        self.tracker
            .poll(
                &self.conn,
                &self.settings,
                &mut source as &mut dyn ProcessSource,
                &self.clock,
            )
            .unwrap()
    }

    fn running(&self) {
        self.source
            .set(&[GAME_EXE, r"c:\windows\explorer.exe"], None);
    }

    fn stopped(&self) {
        self.source.set(&[r"c:\windows\explorer.exe"], None);
    }
}

fn has(events: &[TrackerEvent], f: impl Fn(&TrackerEvent) -> bool) -> bool {
    events.iter().any(f)
}

#[test]
fn starts_and_heartbeats() {
    let mut rig = Rig::new();
    rig.running();
    let events = rig.tick(1000);
    assert!(has(
        &events,
        |e| matches!(e, TrackerEvent::Started(a) if a.game_id == rig.game)
    ));
    assert!(has(&events, |e| matches!(
        e,
        TrackerEvent::StatusChanged(_)
    )));

    let sessions = all_sessions(&rig.conn);
    assert_eq!(sessions.len(), 1);
    let id = sessions[0].session.id;
    assert_eq!(sessions[0].session.source, SessionSource::Auto);
    assert_eq!(repo::last_heartbeat(&rig.conn, id).unwrap(), Some(1000));

    let events = rig.tick(1005);
    assert!(!has(&events, |e| matches!(e, TrackerEvent::Started(_))));
    assert!(has(
        &events,
        |e| matches!(e, TrackerEvent::Updated(a) if a.elapsed_secs == 5)
    ));
    assert_eq!(repo::last_heartbeat(&rig.conn, id).unwrap(), Some(1005));
    assert_eq!(all_sessions(&rig.conn).len(), 1);
}

#[test]
fn ends_after_two_missed_polls_at_last_heartbeat() {
    let mut rig = Rig::new();
    rig.running();
    rig.tick(1000);
    rig.tick(1005);
    rig.stopped();

    let events = rig.tick(1010);
    assert!(
        !has(&events, |e| matches!(e, TrackerEvent::Ended(_))),
        "one miss keeps it open"
    );
    assert!(all_sessions(&rig.conn)[0].session.ended_at.is_none());

    let events = rig.tick(1015);
    assert!(has(
        &events,
        |e| matches!(e, TrackerEvent::Ended(s) if s.ended_at == Some(1005))
    ));
    let s = &all_sessions(&rig.conn)[0].session;
    assert_eq!(s.ended_at, Some(1005));
    assert_eq!(s.duration_secs, 5);
}

#[test]
fn a_single_miss_does_not_end_the_session() {
    let mut rig = Rig::new();
    rig.running();
    rig.tick(1000);
    rig.stopped();
    rig.tick(1005);
    rig.running();
    rig.tick(1010);
    rig.stopped();
    rig.tick(1015);
    assert!(all_sessions(&rig.conn)[0].session.ended_at.is_none());
}

#[test]
fn merges_within_gap_and_splits_outside() {
    let mut rig = Rig::new();
    rig.running();
    rig.tick(1000);
    rig.tick(1005);
    rig.stopped();
    rig.tick(1010);
    rig.tick(1015); // ended at 1005
    let first = all_sessions(&rig.conn)[0].session.id;

    // 1100 is within 120 s of 1005: the first session is reopened.
    rig.running();
    let events = rig.tick(1100);
    assert!(has(
        &events,
        |e| matches!(e, TrackerEvent::Started(a) if a.session_id == first)
    ));
    let sessions = all_sessions(&rig.conn);
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session.ended_at, None);

    // End it again (at heartbeat 1100), then come back after the gap.
    rig.stopped();
    rig.tick(1105);
    rig.tick(1110);
    rig.running();
    rig.tick(1100 + 121 + 1);
    let sessions = all_sessions(&rig.conn);
    assert_eq!(sessions.len(), 2);
    assert_ne!(sessions[0].session.id, first);
}

#[test]
fn manual_session_blocks_auto_session() {
    let mut rig = Rig::new();
    let (view, _) = rig
        .tracker
        .start_manual(&rig.conn, true, rig.game, 1000)
        .unwrap();
    assert!(
        rig.tracker
            .start_manual(&rig.conn, true, rig.game, 1001)
            .is_err(),
        "second manual session for the same game is rejected"
    );

    rig.running();
    let events = rig.tick(1005);
    assert!(!has(&events, |e| matches!(e, TrackerEvent::Started(_))));
    let sessions = all_sessions(&rig.conn);
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session.id, view.session_id);
    assert_eq!(sessions[0].session.source, SessionSource::Manual);

    // The process disappearing does not end a manual session.
    rig.stopped();
    rig.tick(1010);
    rig.tick(1015);
    assert!(all_sessions(&rig.conn)[0].session.ended_at.is_none());
}

#[test]
fn require_foreground_only_counts_the_foreground_process() {
    let mut rig = Rig::new();
    rig.settings.require_foreground = true;
    rig.source.set(
        &[GAME_EXE, r"c:\windows\explorer.exe"],
        Some(r"c:\windows\explorer.exe"),
    );
    rig.tick(1000);
    assert!(all_sessions(&rig.conn).is_empty());

    rig.source.set(&[GAME_EXE], Some(GAME_EXE));
    rig.tick(1005);
    assert_eq!(all_sessions(&rig.conn).len(), 1);
}

#[test]
fn disabling_tracking_ends_auto_but_not_manual_sessions() {
    let mut rig = Rig::new();
    let other = add_game(
        &rig.conn,
        "Bar",
        r"c:\games\bar\bar.exe",
        MatchMode::ExactPath,
    );
    rig.running();
    rig.tick(1000);
    rig.tracker
        .start_manual(&rig.conn, true, other, 1001)
        .unwrap();

    let events = rig
        .tracker
        .set_tracking_enabled(&rig.conn, false, 1002)
        .unwrap();
    assert!(has(
        &events,
        |e| matches!(e, TrackerEvent::Ended(s) if s.ended_at == Some(1002))
    ));
    rig.settings.tracking_enabled = false;
    rig.tick(1005);

    let status = rig.tracker.status(&rig.conn, false, 1005).unwrap();
    assert_eq!(status.active.len(), 1);
    assert_eq!(status.active[0].game_id, other);
    assert!(!status.tracking_enabled);
}

#[test]
fn crash_recovery_closes_open_sessions_at_last_heartbeat() {
    let conn = test_db();
    let game = add_game(&conn, "Foo", GAME_EXE, MatchMode::ExactPath);
    let with_beat =
        repo::insert_session(&conn, game, 100, None, SessionSource::Auto, None).unwrap();
    repo::write_heartbeat(&conn, with_beat.id, 160).unwrap();
    let without_beat =
        repo::insert_session(&conn, game, 500, None, SessionSource::Manual, None).unwrap();

    let closed = gametimetracker_lib::core::tracker::recover_crashed_sessions(&conn).unwrap();
    assert_eq!(closed, 2);
    let a = repo::get_session(&conn, with_beat.id, 9999).unwrap();
    assert_eq!((a.ended_at, a.duration_secs), (Some(160), 60));
    let b = repo::get_session(&conn, without_beat.id, 9999).unwrap();
    assert_eq!((b.ended_at, b.duration_secs), (Some(500), 0));
}

#[test]
fn stopping_a_running_auto_session_is_not_undone_by_the_next_poll() {
    let mut rig = Rig::new();
    rig.running();
    rig.tick(1000);
    let id = all_sessions(&rig.conn)[0].session.id;
    rig.tracker.stop_session(&rig.conn, true, id, 1002).unwrap();
    rig.tick(1005);
    let sessions = all_sessions(&rig.conn);
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session.ended_at, Some(1002));
}

fn steam_game(rig: &Rig, app_id: &str) -> i32 {
    use gametimetracker_lib::core::types::GameSource;
    repo::insert_game(
        &rig.conn,
        "Steam Game",
        GameSource::Steam,
        Some(app_id),
        None,
        0,
    )
    .unwrap()
    .id
}

#[test]
fn matched_games_steam_signal() {
    use gametimetracker_lib::core::tracker::{Snapshot, matched_games};
    let snap = Snapshot {
        processes: vec![],
        foreground: None,
        steam_running_app_id: Some(10),
    };
    assert!(matched_games(&[], &snap, false, Some(7)).contains(&7));
    assert!(matched_games(&[], &snap, false, None).is_empty());
    // Steam cannot say which window is foreground, so the signal waits for a real match.
    assert!(matched_games(&[], &snap, true, Some(7)).is_empty());
}

#[test]
fn snapshot_detects_steam_exe_by_name() {
    use gametimetracker_lib::core::tracker::Snapshot;
    let with = |p: &str| Snapshot {
        processes: vec![p.to_string()],
        ..Snapshot::default()
    };
    assert!(with(r"c:\program files (x86)\steam\steam.exe").steam_running());
    assert!(!with(r"c:\program files (x86)\steam\steamwebhelper.exe").steam_running());
    assert!(!with(r"c:\games\mysteam.exe").steam_running());
}

#[test]
fn steam_running_app_starts_and_ends_a_session_without_a_process_path() {
    let mut rig = Rig::new();
    let game = steam_game(&rig, "1245620");
    let other = steam_game(&rig, "620");

    rig.source.set(&[r"c:\windows\explorer.exe"], None);
    rig.source.set_steam_app(Some(1245620));
    let events = rig.tick(1000);
    assert!(has(
        &events,
        |e| matches!(e, TrackerEvent::Started(a) if a.game_id == game)
    ));
    assert!(!has(
        &events,
        |e| matches!(e, TrackerEvent::Started(a) if a.game_id == other)
    ));

    rig.source.set_steam_app(None);
    rig.tick(1005);
    let events = rig.tick(1010);
    assert!(has(
        &events,
        |e| matches!(e, TrackerEvent::Ended(s) if s.game_id == game)
    ));
}
