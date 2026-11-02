use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

use crate::error::{AppError, AppResult};

fn autostart_err(e: impl std::fmt::Display) -> AppError {
    AppError::Other(format!("autostart: {e}"))
}

#[tauri::command]
#[specta::specta]
pub fn get_autostart(app: AppHandle) -> AppResult<bool> {
    app.autolaunch().is_enabled().map_err(autostart_err)
}

/// Creates or removes the Windows autostart entry (launches with `--minimized`) and returns
/// the resulting state.
#[tauri::command]
#[specta::specta]
pub fn set_autostart(app: AppHandle, enabled: bool) -> AppResult<bool> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(autostart_err)?;
    } else {
        manager.disable().map_err(autostart_err)?;
    }
    manager.is_enabled().map_err(autostart_err)
}
