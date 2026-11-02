use tauri::{AppHandle, State};
use tauri_plugin_store::StoreExt;

use super::now;
use crate::core::settings::backfill_settings;
use crate::core::types::{Settings, SettingsUpdate};
use crate::error::{AppError, AppResult};
use crate::runtime::emit_tracker_events;
use crate::state::AppState;

const SETTINGS_FILE: &str = "settings.json";
const SETTINGS_KEY: &str = "settings";

fn store_err(e: impl std::fmt::Display) -> AppError {
    AppError::Other(format!("settings store: {e}"))
}

/// Reads the persisted settings, falling back to defaults when missing or invalid.
pub fn load_settings(app: &AppHandle) -> Settings {
    let stored = app
        .store(SETTINGS_FILE)
        .ok()
        .and_then(|store| store.get(SETTINGS_KEY))
        .map(backfill_settings)
        .and_then(|value| serde_json::from_value::<Settings>(value).ok())
        .filter(|s| s.validate().is_ok());
    stored.unwrap_or_default()
}

fn persist(app: &AppHandle, settings: &Settings) -> AppResult<()> {
    let store = app.store(SETTINGS_FILE).map_err(store_err)?;
    store.set(
        SETTINGS_KEY,
        serde_json::to_value(settings).map_err(store_err)?,
    );
    store.save().map_err(store_err)
}

/// Validates and applies a patch: persists it, refreshes the cached copy, and ends auto
/// sessions when tracking was switched off.
pub fn apply_settings(
    app: &AppHandle,
    state: &AppState,
    patch: &SettingsUpdate,
) -> AppResult<Settings> {
    let current = state.settings();
    let next = current.with_update(patch)?;
    persist(app, &next)?;
    *state
        .settings
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = next.clone();
    if next.tracking_enabled != current.tracking_enabled {
        let events = {
            let mut tracker = state.tracker();
            let db = state.db();
            tracker.set_tracking_enabled(&db, next.tracking_enabled, now())?
        };
        emit_tracker_events(app, events);
    }
    Ok(next)
}

#[tauri::command]
#[specta::specta]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings()
}

#[tauri::command]
#[specta::specta]
pub fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    patch: SettingsUpdate,
) -> AppResult<Settings> {
    apply_settings(&app, &state, &patch)
}
