use std::time::Duration;

use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::{Update, UpdaterExt};
use time::format_description::well_known::Rfc3339;

use crate::core::types::{UpdateInfo, UpdateProgress};
use crate::error::{AppError, AppResult};

/// Delay before the silent check that follows launch.
const STARTUP_CHECK_DELAY: Duration = Duration::from_secs(10);

fn update_err(e: impl std::fmt::Display) -> AppError {
    AppError::Other(format!("update: {e}"))
}

/// Release notes with blank text treated as absent.
fn clean_notes(body: Option<String>) -> Option<String> {
    body.filter(|b| !b.trim().is_empty())
}

fn to_info(update: &Update) -> UpdateInfo {
    UpdateInfo {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        date: update.date.and_then(|d| d.format(&Rfc3339).ok()),
        notes: clean_notes(update.body.clone()),
    }
}

async fn find_update(app: &AppHandle) -> AppResult<Option<Update>> {
    app.updater()
        .map_err(update_err)?
        .check()
        .await
        .map_err(update_err)
}

/// Queries the release feed. `None` when the app is up to date.
#[tauri::command]
#[specta::specta]
pub async fn check_for_update(app: AppHandle) -> AppResult<Option<UpdateInfo>> {
    Ok(find_update(&app).await?.as_ref().map(to_info))
}

/// Downloads, verifies and installs the newest release, then relaunches. Emits
/// `update-progress` while downloading. On Windows the installer ends the process itself.
#[tauri::command]
#[specta::specta]
pub async fn install_update(app: AppHandle) -> AppResult<()> {
    let update = find_update(&app)
        .await?
        .ok_or_else(|| AppError::Other("update: already up to date".into()))?;
    let mut downloaded: u64 = 0;
    let progress_app = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = progress_app.emit("update-progress", UpdateProgress { downloaded, total });
            },
            || tracing::info!("update downloaded, installing"),
        )
        .await
        .map_err(update_err)?;
    app.restart()
}

/// Checks once, about 10 seconds after launch, and emits `update-available` when a newer
/// version exists. Failures (offline, no release yet) are only logged at debug level.
pub fn spawn_startup_update_check(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(STARTUP_CHECK_DELAY);
        match tauri::async_runtime::block_on(find_update(&app)) {
            Ok(Some(update)) => {
                tracing::info!(version = %update.version, "update available");
                if let Err(e) = app.emit("update-available", to_info(&update)) {
                    tracing::warn!(error = %e, "failed to emit update-available");
                }
            }
            Ok(None) => tracing::debug!("startup update check: up to date"),
            Err(e) => tracing::debug!(error = %e, "startup update check failed"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::clean_notes;

    #[test]
    fn blank_notes_become_none() {
        assert_eq!(clean_notes(None), None);
        assert_eq!(clean_notes(Some("  \n".into())), None);
        assert_eq!(
            clean_notes(Some("Fixed things".into())).as_deref(),
            Some("Fixed things")
        );
    }
}
