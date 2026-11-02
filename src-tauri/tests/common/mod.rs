#![allow(dead_code)]

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use gamelapse_lib::core::db::{migrations, repo};
use gamelapse_lib::core::idle::IdleReading;
use gamelapse_lib::core::tracker::{Clock, ProcessSource, Snapshot, Tracker, TrackerEvent};
use gamelapse_lib::core::types::{ActiveSession, GameSource, MatchMode, Session, Settings};
use rusqlite::Connection;

pub fn test_db() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    migrations::run(&mut conn).unwrap();
    conn
}

/// Inserts a game with one executable rule and returns its id.
pub fn add_game(conn: &Connection, name: &str, exe: &str, mode: MatchMode) -> i32 {
    let game = repo::insert_game(conn, name, GameSource::Manual, None, None, 0).unwrap();
    repo::insert_executable(conn, game.id, exe, mode).unwrap();
    game.id
}

#[derive(Clone, Default)]
pub struct FakeSource(pub Arc<Mutex<Snapshot>>);

impl FakeSource {
    pub fn set(&self, processes: &[&str], foreground: Option<&str>) {
        *self.0.lock().unwrap() = Snapshot {
            processes: processes.iter().map(|p| p.to_string()).collect(),
            foreground: foreground.map(str::to_string),
            steam_running_app_id: None,
        };
    }

    pub fn set_steam_app(&self, app_id: Option<u32>) {
        self.0.lock().unwrap().steam_running_app_id = app_id;
    }
}

impl ProcessSource for FakeSource {
    fn snapshot(&mut self) -> Snapshot {
        self.0.lock().unwrap().clone()
    }
}

pub const FOO_EXE: &str = r"c:\games\foo\foo.exe";
pub const BAR_EXE: &str = r"c:\games\bar\bar.exe";

/// A tracker with a database, a fake process list and settings, driven tick by tick with an
/// explicit idle reading. Game "Foo" is created up front.
pub struct TrackRig {
    pub conn: Connection,
    pub tracker: Tracker,
    pub source: FakeSource,
    pub settings: Settings,
    pub game: i32,
}

impl TrackRig {
    pub fn new() -> Self {
        let conn = test_db();
        let game = add_game(&conn, "Foo", FOO_EXE, MatchMode::ExactPath);
        let source = FakeSource::default();
        source.set(&[FOO_EXE], None);
        Self {
            conn,
            tracker: Tracker::new(),
            source,
            settings: Settings::default(),
            game,
        }
    }

    pub fn tick_with(&mut self, now: i64, idle: IdleReading) -> Vec<TrackerEvent> {
        let snapshot = self
            .settings
            .tracking_enabled
            .then(|| self.source.snapshot());
        self.tracker
            .tick(&self.conn, &self.settings, snapshot.as_ref(), idle, now)
            .unwrap()
    }

    /// Tick with `idle_secs` since the last input and an unlocked workstation.
    pub fn tick(&mut self, now: i64, idle_secs: i64) -> Vec<TrackerEvent> {
        self.tick_with(
            now,
            IdleReading {
                idle_secs: Some(idle_secs),
                locked: false,
            },
        )
    }

    /// The last `Updated` payload for a game in a batch of events.
    pub fn updated(events: &[TrackerEvent], game: i32) -> ActiveSession {
        events
            .iter()
            .rev()
            .find_map(|e| match e {
                TrackerEvent::Updated(a) if a.game_id == game => Some(a.clone()),
                _ => None,
            })
            .expect("an Updated event for the game")
    }

    pub fn session(&self, id: i32, now: i64) -> Session {
        repo::get_session(&self.conn, id, now).unwrap()
    }
}

#[derive(Clone)]
pub struct FakeClock(pub Arc<AtomicI64>);

impl FakeClock {
    pub fn new(start: i64) -> Self {
        Self(Arc::new(AtomicI64::new(start)))
    }
    pub fn set(&self, t: i64) {
        self.0.store(t, Ordering::SeqCst);
    }
}

impl Clock for FakeClock {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}
