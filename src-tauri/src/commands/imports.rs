//! Launcher import commands. Scanning, downloads and HTTP run on the blocking pool, never on
//! the main thread, and the database lock is only held for the final writes.

use serde_json::json;
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;

use super::now;
use crate::core::db::repo;
use crate::core::importers::{self, covers, http, steam_api, store};
use crate::core::types::{
    DiscoveredGame, GameSource, IgnoredImport, ImportResult, ImportSource, ImportSourceStatus,
    SteamApiSettings,
};
use crate::error::{AppError, AppResult};
use crate::runtime::emit_library_changed;
use crate::state::AppState;

const SETTINGS_FILE: &str = "settings.json";
const STEAM_API_KEY: &str = "steamApi";

fn store_err(e: impl std::fmt::Display) -> AppError {
    AppError::Other(format!("settings store: {e}"))
}

async fn blocking<T, F>(f: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> AppResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::Other(format!("background task failed: {e}")))?
}

/// Statuses of all three launchers, with their last import time.
pub fn source_statuses(app: &AppHandle) -> AppResult<Vec<ImportSourceStatus>> {
    let state = app.state::<AppState>();
    let mut statuses = Vec::new();
    for source in ImportSource::ALL {
        let mut status = importers::importer_for(source).status();
        status.last_scan_at = importers::last_scan_at(&state.db(), source)?;
        statuses.push(status);
    }
    Ok(statuses)
}

/// Scans `source` and upserts the games. Emits `library-changed`. Used by the command and
/// by the startup import.
pub fn run_import(app: &AppHandle, source: ImportSource) -> AppResult<ImportResult> {
    let state = app.state::<AppState>();
    let importer = importers::importer_for(source);
    let scan = importer.scan()?;
    let covers = if source == ImportSource::Steam {
        covers::ensure_steam_covers(&state.data_dir, &scan.games, &covers::http_download)
    } else {
        covers::ensure_url_covers(&state.data_dir, &scan.covers, &covers::http_download)
    };
    let result = {
        let db = state.db();
        store::import_games(&db, source, &scan.games, &covers, scan.errors, now())?
    };
    emit_library_changed(app);
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn list_import_sources(app: AppHandle) -> AppResult<Vec<ImportSourceStatus>> {
    blocking(move || source_statuses(&app)).await
}

#[tauri::command]
#[specta::specta]
pub async fn scan_import_source(
    app: AppHandle,
    source: ImportSource,
) -> AppResult<Vec<DiscoveredGame>> {
    blocking(move || {
        let scan = importers::importer_for(source).scan()?;
        for e in &scan.errors {
            tracing::warn!(source = source.as_str(), error = %e, "scan problem");
        }
        let mut games = scan.games;
        importers::mark_imported(&app.state::<AppState>().db(), &mut games)?;
        Ok(games)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn import_games(app: AppHandle, source: ImportSource) -> AppResult<ImportResult> {
    blocking(move || run_import(&app, source)).await
}

#[tauri::command]
#[specta::specta]
pub fn list_ignored_imports(state: tauri::State<'_, AppState>) -> AppResult<Vec<IgnoredImport>> {
    repo::list_ignored_imports(&state.db())
}

#[tauri::command]
#[specta::specta]
pub fn restore_ignored_import(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    source: GameSource,
    source_id: String,
) -> AppResult<()> {
    repo::restore_ignored_import(&state.db(), source, &source_id)?;
    emit_library_changed(&app);
    Ok(())
}

fn load_steam_api(app: &AppHandle) -> AppResult<SteamApiSettings> {
    let store = app.store(SETTINGS_FILE).map_err(store_err)?;
    Ok(store
        .get(STEAM_API_KEY)
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default())
}

#[tauri::command]
#[specta::specta]
pub fn get_steam_api_settings(app: AppHandle) -> AppResult<SteamApiSettings> {
    load_steam_api(&app)
}

#[tauri::command]
#[specta::specta]
pub fn set_steam_api_settings(app: AppHandle, settings: SteamApiSettings) -> AppResult<()> {
    let clean = |v: Option<String>| v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let settings = SteamApiSettings {
        api_key: clean(settings.api_key),
        steam_id64: clean(settings.steam_id64),
    };
    let store = app.store(SETTINGS_FILE).map_err(store_err)?;
    store.set(
        STEAM_API_KEY,
        json!({ "apiKey": settings.api_key, "steamId64": settings.steam_id64 }),
    );
    store.save().map_err(store_err)
}

#[tauri::command]
#[specta::specta]
pub async fn fetch_steam_playtime(app: AppHandle) -> AppResult<ImportResult> {
    blocking(move || {
        let settings = load_steam_api(&app)?;
        let owned = steam_api::fetch_owned_games(&settings, &|url| {
            http::get(url).map(|r| (r.status, String::from_utf8_lossy(&r.body).into_owned()))
        })?;
        let result = {
            let state = app.state::<AppState>();
            let db = state.db();
            steam_api::apply_owned_games(&db, &owned, now())?
        };
        emit_library_changed(&app);
        Ok(result)
    })
    .await
}
