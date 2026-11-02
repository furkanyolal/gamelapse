use std::path::PathBuf;

use chrono::{Local, TimeZone};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use super::now;
use crate::core::db::{self, repo};
use crate::core::export;
use crate::core::tracker::{Tracker, recover_crashed_sessions};
use crate::core::types::{BackupInfo, ExportFormat, ExportOptions, ExportResult};
use crate::error::{AppError, AppResult};
use crate::runtime::{emit_library_changed, emit_sessions_changed};
use crate::state::AppState;

fn dialog_err(e: impl std::fmt::Display) -> AppError {
    AppError::Other(format!("file dialog failed: {e}"))
}

/// Native save dialog on a blocking thread. `None` when cancelled.
async fn pick_save_path(
    app: AppHandle,
    file_name: String,
    filter_name: &'static str,
    extensions: &'static [&'static str],
) -> AppResult<Option<PathBuf>> {
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_file_name(file_name)
            .add_filter(filter_name, extensions)
            .blocking_save_file()
    })
    .await
    .map_err(dialog_err)?;
    Ok(picked.and_then(|p| p.into_path().ok()))
}

#[tauri::command]
#[specta::specta]
pub async fn export_data(
    app: AppHandle,
    state: State<'_, AppState>,
    options: ExportOptions,
) -> AppResult<Option<ExportResult>> {
    let ts = now();
    let name = export::default_file_name(options.format, ts);
    let (filter, extensions): (&'static str, &'static [&'static str]) = match options.format {
        ExportFormat::Csv => ("CSV", &["csv"]),
        ExportFormat::Json => ("JSON", &["json"]),
    };
    let Some(path) = pick_save_path(app, name, filter, extensions).await? else {
        return Ok(None);
    };
    let db = state.db();
    Ok(Some(export::export_to_path(&db, &options, &path, ts)?))
}

#[tauri::command]
#[specta::specta]
pub async fn backup_database(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<Option<BackupInfo>> {
    let ts = now();
    let stamp = Local
        .timestamp_opt(ts, 0)
        .earliest()
        .unwrap_or_else(Local::now)
        .format("%Y%m%d-%H%M");
    let name = format!("gametimetracker-backup-{stamp}.db");
    let Some(path) = pick_save_path(app, name, "Database", &["db"]).await? else {
        return Ok(None);
    };
    {
        let db = state.db();
        db::backup_to(&db, &path)?;
    }
    let size = std::fs::metadata(&path)?.len();
    Ok(Some(BackupInfo {
        path: path.display().to_string(),
        created_at: ts,
        size_bytes: i64::try_from(size).unwrap_or(i64::MAX),
    }))
}

/// Pauses the tracker loop for as long as it lives, even on early return.
struct PauseGuard<'a>(&'a AppState);

impl<'a> PauseGuard<'a> {
    fn new(state: &'a AppState) -> Self {
        state
            .paused
            .store(true, std::sync::atomic::Ordering::SeqCst);
        Self(state)
    }
}

impl Drop for PauseGuard<'_> {
    fn drop(&mut self) {
        self.0
            .paused
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

fn ensure_no_active_session(state: &AppState) -> AppResult<()> {
    let tracker = state.tracker();
    let db = state.db();
    if tracker.has_active() || repo::has_open_sessions(&db)? {
        return Err(AppError::Other(
            "a session is active; stop it before restoring a backup".into(),
        ));
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn restore_database(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    ensure_no_active_session(&state)?;
    let picker_app = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        picker_app
            .dialog()
            .file()
            .add_filter("Database", &["db"])
            .blocking_pick_file()
    })
    .await
    .map_err(dialog_err)?;
    let Some(backup) = picked.and_then(|p| p.into_path().ok()) else {
        return Ok(());
    };
    if backup == state.db_path {
        return Err(AppError::Other(
            "choose a backup file, not the live database".into(),
        ));
    }
    db::validate_backup(&backup)?;

    {
        let _pause = PauseGuard::new(&state);
        // Lock order: tracker, then db. Holding both keeps the poll loop and commands out.
        let mut tracker = state.tracker();
        let mut conn = state.db();
        if tracker.has_active() || repo::has_open_sessions(&conn)? {
            return Err(AppError::Other(
                "a session is active; stop it before restoring a backup".into(),
            ));
        }
        db::replace_database(&mut conn, &state.db_path, &backup)?;
        match recover_crashed_sessions(&conn) {
            Ok(0) => {}
            Ok(n) => tracing::info!(
                closed = n,
                "closed sessions left open in the restored backup"
            ),
            Err(e) => tracing::error!(error = %e, "crash recovery after restore failed"),
        }
        *tracker = Tracker::new();
    }
    tracing::info!(backup = %backup.display(), "database restored");
    emit_library_changed(&app);
    emit_sessions_changed(&app);
    Ok(())
}
