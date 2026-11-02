//! Shared application state managed by Tauri.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError, RwLock};

use rusqlite::Connection;
use tracing_appender::non_blocking::WorkerGuard;

use crate::core::tracker::Tracker;
use crate::core::types::Settings;

/// Lock order: `tracker` before `db`. Never hold either while sleeping.
/// `settings` is read-cloned and never held across other locks.
pub struct AppState {
    pub db: Mutex<Connection>,
    pub tracker: Mutex<Tracker>,
    /// Cached copy of the persisted settings, read by the tracker on every poll.
    pub settings: RwLock<Settings>,
    /// Set while the database file is being swapped; the poll loop skips its tick.
    pub paused: AtomicBool,
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub log_dir: PathBuf,
}

impl AppState {
    pub fn db(&self) -> MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn tracker(&self) -> MutexGuard<'_, Tracker> {
        self.tracker.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    pub fn settings(&self) -> Settings {
        self.settings
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// Keeps the non-blocking log writer alive for the lifetime of the app.
#[allow(dead_code)]
pub struct LogGuard(pub WorkerGuard);
