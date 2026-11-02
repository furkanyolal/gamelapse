use tauri::{AppHandle, State};

use super::now;
use crate::core::db::repo;
use crate::core::types::{Session, SessionQuery, SessionUpdate, SessionWithGame, Ts};
use crate::error::AppResult;
use crate::runtime::emit_sessions_changed;
use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub fn list_sessions(
    state: State<'_, AppState>,
    query: SessionQuery,
) -> AppResult<Vec<SessionWithGame>> {
    repo::list_sessions(&state.db(), &query, now())
}

#[tauri::command]
#[specta::specta]
pub fn create_session(
    app: AppHandle,
    state: State<'_, AppState>,
    game_id: i32,
    started_at: Ts,
    ended_at: Ts,
    note: Option<String>,
) -> AppResult<Session> {
    let session = repo::create_session(
        &state.db(),
        game_id,
        started_at.0,
        ended_at.0,
        note.as_deref(),
    )?;
    emit_sessions_changed(&app);
    Ok(session)
}

#[tauri::command]
#[specta::specta]
pub fn update_session(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: i32,
    patch: SessionUpdate,
) -> AppResult<Session> {
    let session = repo::update_session(&state.db(), session_id, &patch, now())?;
    emit_sessions_changed(&app);
    Ok(session)
}

#[tauri::command]
#[specta::specta]
pub fn delete_session(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: i32,
) -> AppResult<()> {
    repo::delete_session(&state.db(), session_id, now())?;
    emit_sessions_changed(&app);
    Ok(())
}
