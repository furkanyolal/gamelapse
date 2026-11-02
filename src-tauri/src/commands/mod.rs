//! Tauri commands exposed to the frontend.

use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::error::AppResult;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Type)]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
    pub db_path: String,
    pub log_dir: String,
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
