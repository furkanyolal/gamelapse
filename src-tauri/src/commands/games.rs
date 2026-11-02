use std::path::Path;

use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use super::now;
use crate::core::db::{repo, stats};
use crate::core::types::{Game, GameExecutable, GameSource, GameSummary, GameUpdate, MatchMode};
use crate::error::{AppError, AppResult};
use crate::runtime::emit_library_changed;
use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub fn list_games(state: State<'_, AppState>, include_hidden: bool) -> AppResult<Vec<GameSummary>> {
    let db = state.db();
    stats::list_game_summaries(&db, include_hidden, now())
}

#[tauri::command]
#[specta::specta]
pub fn get_game(state: State<'_, AppState>, game_id: i32) -> AppResult<GameSummary> {
    let db = state.db();
    let game = repo::get_game(&db, game_id)?;
    stats::game_summary(&db, game, now())
}

#[tauri::command]
#[specta::specta]
pub fn add_game_from_exe(
    app: AppHandle,
    state: State<'_, AppState>,
    exe_path: String,
    name: Option<String>,
) -> AppResult<Game> {
    let path = Path::new(&exe_path);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::Other("not a valid executable path".into()))?;
    let name = name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or(stem);
    let install_dir = path
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .filter(|p| !p.is_empty());

    let game = {
        let db = state.db();
        if repo::executable_attached(&db, &exe_path)? {
            return Err(AppError::Other(
                "that executable is already attached to a game".into(),
            ));
        }
        let tx = db.unchecked_transaction()?;
        let game = repo::insert_game(
            &tx,
            &name,
            GameSource::Manual,
            None,
            install_dir.as_deref(),
            now(),
        )?;
        repo::insert_executable(&tx, game.id, &exe_path, MatchMode::ExactPath)?;
        tx.commit()?;
        game
    };
    emit_library_changed(&app);
    Ok(game)
}

#[tauri::command]
#[specta::specta]
pub fn update_game(
    app: AppHandle,
    state: State<'_, AppState>,
    game_id: i32,
    patch: GameUpdate,
) -> AppResult<Game> {
    let game = repo::update_game(&state.db(), game_id, &patch)?;
    emit_library_changed(&app);
    Ok(game)
}

#[tauri::command]
#[specta::specta]
pub fn delete_game(app: AppHandle, state: State<'_, AppState>, game_id: i32) -> AppResult<()> {
    {
        // Holding the tracker lock keeps a poll from starting a session mid-delete.
        let _tracker = state.tracker();
        let db = state.db();
        repo::delete_game(&db, game_id, now())?;
    }
    emit_library_changed(&app);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn list_game_executables(
    state: State<'_, AppState>,
    game_id: i32,
) -> AppResult<Vec<GameExecutable>> {
    repo::list_executables(&state.db(), game_id)
}

#[tauri::command]
#[specta::specta]
pub fn add_game_executable(
    app: AppHandle,
    state: State<'_, AppState>,
    game_id: i32,
    exe_path: String,
    match_mode: MatchMode,
) -> AppResult<GameExecutable> {
    let exe = repo::insert_executable(&state.db(), game_id, &exe_path, match_mode)?;
    emit_library_changed(&app);
    Ok(exe)
}

#[tauri::command]
#[specta::specta]
pub fn remove_game_executable(
    app: AppHandle,
    state: State<'_, AppState>,
    executable_id: i32,
) -> AppResult<()> {
    repo::remove_executable(&state.db(), executable_id)?;
    emit_library_changed(&app);
    Ok(())
}

/// Native file picker. Async so the blocking dialog never runs on the main thread.
#[tauri::command]
#[specta::specta]
pub async fn pick_executable(app: AppHandle) -> AppResult<Option<String>> {
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("Executable", &["exe"])
            .blocking_pick_file()
    })
    .await
    .map_err(|e| AppError::Other(format!("file dialog failed: {e}")))?;
    Ok(picked
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned()))
}
