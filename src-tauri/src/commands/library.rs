use std::path::Path;

use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::core::custom_covers;
use crate::core::db::{library, repo};
use crate::core::types::{Game, MergeResult, Tag};
use crate::error::{AppError, AppResult};
use crate::runtime::emit_library_changed;
use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub fn list_tags(state: State<'_, AppState>) -> AppResult<Vec<Tag>> {
    library::list_tags(&state.db())
}

#[tauri::command]
#[specta::specta]
pub fn create_tag(app: AppHandle, state: State<'_, AppState>, name: String) -> AppResult<Tag> {
    let tag = library::create_tag(&state.db(), &name)?;
    emit_library_changed(&app);
    Ok(tag)
}

#[tauri::command]
#[specta::specta]
pub fn rename_tag(
    app: AppHandle,
    state: State<'_, AppState>,
    tag_id: i32,
    name: String,
) -> AppResult<Tag> {
    let tag = library::rename_tag(&state.db(), tag_id, &name)?;
    emit_library_changed(&app);
    Ok(tag)
}

#[tauri::command]
#[specta::specta]
pub fn delete_tag(app: AppHandle, state: State<'_, AppState>, tag_id: i32) -> AppResult<()> {
    library::delete_tag(&state.db(), tag_id)?;
    emit_library_changed(&app);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn set_game_tags(
    app: AppHandle,
    state: State<'_, AppState>,
    game_id: i32,
    tag_ids: Vec<i32>,
) -> AppResult<Vec<Tag>> {
    let tags = library::set_game_tags(&state.db(), game_id, &tag_ids)?;
    emit_library_changed(&app);
    Ok(tags)
}

#[tauri::command]
#[specta::specta]
pub fn merge_games(
    app: AppHandle,
    state: State<'_, AppState>,
    survivor_id: i32,
    merged_ids: Vec<i32>,
) -> AppResult<MergeResult> {
    let result = {
        // Holding the tracker lock keeps a poll from starting a session mid-merge.
        let _tracker = state.tracker();
        let db = state.db();
        library::merge_games(&db, survivor_id, &merged_ids, super::now())?
    };
    for id in &merged_ids {
        custom_covers::remove(&state.data_dir, *id);
    }
    emit_library_changed(&app);
    Ok(result)
}

/// Native image picker. Async so the blocking dialog never runs on the main thread.
#[tauri::command]
#[specta::specta]
pub async fn pick_image(app: AppHandle) -> AppResult<Option<String>> {
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("Image", &custom_covers::EXTENSIONS)
            .blocking_pick_file()
    })
    .await
    .map_err(|e| AppError::Other(format!("file dialog failed: {e}")))?;
    Ok(picked
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned()))
}

#[tauri::command]
#[specta::specta]
pub fn set_game_cover(
    app: AppHandle,
    state: State<'_, AppState>,
    game_id: i32,
    source_path: Option<String>,
) -> AppResult<Game> {
    let db = state.db();
    repo::get_game(&db, game_id)?;
    let game = match source_path {
        Some(source) => {
            let dest = custom_covers::set(&state.data_dir, game_id, Path::new(&source))?;
            repo::set_cover_path(&db, game_id, Some(&dest.to_string_lossy()))?
        }
        None => {
            let game = repo::set_cover_path(&db, game_id, None)?;
            custom_covers::remove(&state.data_dir, game_id);
            game
        }
    };
    drop(db);
    emit_library_changed(&app);
    Ok(game)
}
