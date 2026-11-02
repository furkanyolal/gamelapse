use tauri::{AppHandle, State};

use super::now;
use crate::core::types::{ActiveSession, Session, SettingsUpdate, TrackerStatus};
use crate::error::AppResult;
use crate::runtime::emit_tracker_events;
use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub fn get_tracker_status(state: State<'_, AppState>) -> AppResult<TrackerStatus> {
    let enabled = state.settings().tracking_enabled;
    let tracker = state.tracker();
    let db = state.db();
    tracker.status(&db, enabled, now())
}

#[tauri::command]
#[specta::specta]
pub fn set_tracking_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> AppResult<TrackerStatus> {
    super::settings::apply_settings(
        &app,
        &state,
        &SettingsUpdate {
            tracking_enabled: Some(enabled),
            ..Default::default()
        },
    )?;
    let tracker = state.tracker();
    let db = state.db();
    tracker.status(&db, enabled, now())
}

#[tauri::command]
#[specta::specta]
pub fn start_manual_session(
    app: AppHandle,
    state: State<'_, AppState>,
    game_id: i32,
) -> AppResult<ActiveSession> {
    let enabled = state.settings().tracking_enabled;
    let (view, events) = {
        let mut tracker = state.tracker();
        let db = state.db();
        tracker.start_manual(&db, enabled, game_id, now())?
    };
    emit_tracker_events(&app, events);
    Ok(view)
}

#[tauri::command]
#[specta::specta]
pub fn stop_session(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: i32,
) -> AppResult<Session> {
    let enabled = state.settings().tracking_enabled;
    let (session, events) = {
        let mut tracker = state.tracker();
        let db = state.db();
        tracker.stop_session(&db, enabled, session_id, now())?
    };
    emit_tracker_events(&app, events);
    Ok(session)
}
