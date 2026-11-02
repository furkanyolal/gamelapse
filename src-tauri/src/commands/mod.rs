//! Tauri commands exposed to the frontend, grouped by family.
//! Lock order: tracker, then db (see `AppState`).

pub mod dev;
pub mod games;
pub mod imports;
pub mod sessions;
pub mod settings;
pub mod stats;
pub mod tracker;

use tauri::State;

use crate::core::tracker::{Clock, SystemClock};
use crate::core::types::AppInfo;
use crate::error::AppResult;
use crate::state::AppState;

pub(crate) fn now() -> i64 {
    SystemClock.now()
}

#[tauri::command]
#[specta::specta]
pub fn get_app_info(state: State<'_, AppState>) -> AppResult<AppInfo> {
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        data_dir: state.data_dir.display().to_string(),
        db_path: state.db_path.display().to_string(),
        log_dir: state.log_dir.display().to_string(),
    })
}

#[tauri::command]
#[specta::specta]
pub fn ping() -> String {
    "pong".to_string()
}
