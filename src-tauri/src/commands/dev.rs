use tauri::{AppHandle, State};

use super::now;
use crate::core::db::seed;
use crate::error::AppResult;
use crate::runtime::{emit_library_changed, emit_sessions_changed};
use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub fn seed_demo_data(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    let inserted = seed::seed_demo_data(&state.db(), now())?;
    if inserted {
        emit_library_changed(&app);
        emit_sessions_changed(&app);
    }
    Ok(())
}
