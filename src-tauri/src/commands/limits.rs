use tauri::State;

use super::now;
use crate::core::limits;
use crate::core::types::{Limit, LimitInput, LimitStatus};
use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub fn list_limits(state: State<'_, AppState>) -> AppResult<Vec<LimitStatus>> {
    limits::list_statuses(&state.db(), now())
}

#[tauri::command]
#[specta::specta]
pub fn create_limit(state: State<'_, AppState>, input: LimitInput) -> AppResult<Limit> {
    limits::create_limit(&state.db(), &input)
}

#[tauri::command]
#[specta::specta]
pub fn update_limit(
    state: State<'_, AppState>,
    limit_id: i32,
    input: LimitInput,
) -> AppResult<Limit> {
    limits::update_limit(&state.db(), limit_id, &input)
}

#[tauri::command]
#[specta::specta]
pub fn delete_limit(state: State<'_, AppState>, limit_id: i32) -> AppResult<()> {
    limits::delete_limit(&state.db(), limit_id)
}
