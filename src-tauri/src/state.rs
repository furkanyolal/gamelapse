//! Shared application state managed by Tauri.

use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;
use tracing_appender::non_blocking::WorkerGuard;

pub struct AppState {
    pub db: Mutex<Connection>,
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub log_dir: PathBuf,
}

/// Keeps the non-blocking log writer alive for the lifetime of the app.
#[allow(dead_code)]
pub struct LogGuard(pub WorkerGuard);
