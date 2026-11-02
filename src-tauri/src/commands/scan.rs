//! Folder scan and metadata lookup commands. Scanning, downloads and HTTP run on the
//! blocking pool. A scan happens only inside `scan_folders`, and nothing reaches the
//! library except through `add_scanned_games` and `apply_game_metadata`.

use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use super::now;
use crate::core::db::repo;
use crate::core::importers::covers::http_download;
use crate::core::metadata::{self, UreqHttp};
use crate::core::scan::{self, add, roots};
use crate::core::types::{
    Game, ImportResult, MetadataMatch, ScanCandidate, ScanRoot, ScanSelection,
};
use crate::error::{AppError, AppResult};
use crate::runtime::emit_library_changed;
use crate::state::AppState;

async fn blocking<T, F>(f: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> AppResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::Other(format!("background task failed: {e}")))?
}

/// Native folder picker. Async so the blocking dialog never runs on the main thread.
#[tauri::command]
#[specta::specta]
pub async fn pick_folder(app: AppHandle) -> AppResult<Option<String>> {
    let picked =
        tauri::async_runtime::spawn_blocking(move || app.dialog().file().blocking_pick_folder())
            .await
            .map_err(|e| AppError::Other(format!("file dialog failed: {e}")))?;
    Ok(picked
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned()))
}

#[tauri::command]
#[specta::specta]
pub fn list_scan_roots(state: State<'_, AppState>) -> AppResult<Vec<ScanRoot>> {
    roots::list(&state.db())
}

#[tauri::command]
#[specta::specta]
pub fn add_scan_root(state: State<'_, AppState>, path: String) -> AppResult<ScanRoot> {
    roots::add(&state.db(), &path, now())
}

#[tauri::command]
#[specta::specta]
pub fn remove_scan_root(state: State<'_, AppState>, root_id: i32) -> AppResult<()> {
    roots::remove(&state.db(), root_id)
}

/// Scans the chosen roots (`None`: all). Only reads; the walk holds no database lock.
#[tauri::command]
#[specta::specta]
pub async fn scan_folders(
    app: AppHandle,
    root_ids: Option<Vec<i32>>,
) -> AppResult<Vec<ScanCandidate>> {
    blocking(move || {
        let state = app.state::<AppState>();
        let (selected, rules) = {
            let db = state.db();
            let mut selected = roots::list(&db)?;
            if let Some(ids) = &root_ids {
                selected.retain(|r| ids.contains(&r.id));
            }
            (selected, repo::list_all_executables(&db)?)
        };
        Ok(scan::scan_roots(&selected, &rules))
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn add_scanned_games(
    app: AppHandle,
    selection: Vec<ScanSelection>,
) -> AppResult<ImportResult> {
    blocking(move || {
        let state = app.state::<AppState>();
        let result = add::add_scanned_games(
            &state.db,
            &state.data_dir,
            &selection,
            &http_download,
            now(),
        )?;
        emit_library_changed(&app);
        Ok(result)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn lookup_game_metadata(name: String) -> AppResult<Vec<MetadataMatch>> {
    blocking(move || metadata::lookup_game_metadata(&UreqHttp, &name)).await
}

#[tauri::command]
#[specta::specta]
pub async fn apply_game_metadata(
    app: AppHandle,
    game_id: i32,
    found: MetadataMatch,
    rename_game: bool,
) -> AppResult<Game> {
    blocking(move || {
        let state = app.state::<AppState>();
        let game = metadata::apply_game_metadata(
            &state.db,
            &state.data_dir,
            game_id,
            &found,
            rename_game,
            &http_download,
        )?;
        emit_library_changed(&app);
        Ok(game)
    })
    .await
}
