#![allow(dead_code)]

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use gametimetracker_lib::core::db::{migrations, repo};
use gametimetracker_lib::core::tracker::{Clock, ProcessSource, Snapshot};
use gametimetracker_lib::core::types::{GameSource, MatchMode};
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
