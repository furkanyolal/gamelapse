use tauri::State;

use super::now;
use crate::core::db::stats;
use crate::core::types::{DailyTotal, DashboardStats, GameTotal, HourlyTotal, Ts};
use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub fn get_dashboard_stats(state: State<'_, AppState>) -> AppResult<DashboardStats> {
    stats::dashboard(&state.db(), now())
}

#[tauri::command]
#[specta::specta]
pub fn get_daily_totals(
    state: State<'_, AppState>,
    days: u32,
    game_id: Option<i32>,
) -> AppResult<Vec<DailyTotal>> {
    stats::daily_totals(&state.db(), now(), days, game_id)
}

#[tauri::command]
#[specta::specta]
pub fn get_game_totals(
    state: State<'_, AppState>,
    from: Option<Ts>,
    to: Option<Ts>,
    limit: u32,
) -> AppResult<Vec<GameTotal>> {
    stats::game_totals(
        &state.db(),
        now(),
        from.map(|t| t.0),
        to.map(|t| t.0),
        limit,
    )
}

#[tauri::command]
#[specta::specta]
pub fn get_hourly_totals(state: State<'_, AppState>, days: u32) -> AppResult<Vec<HourlyTotal>> {
    stats::hourly_totals(&state.db(), now(), days)
}
