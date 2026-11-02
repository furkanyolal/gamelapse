//! Glue between the tracker core and Tauri: the polling thread and event emission.

use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::core::idle::{IdleReading, IdleSource, WinIdleSource};
use crate::core::tracker::sysinfo_source::SysinfoProcessSource;
use crate::core::tracker::{Clock, ProcessSource, SystemClock, TrackerEvent};
use crate::core::types::{LimitScope, LimitStatus, Reminder};
use crate::state::AppState;

const SLEEP_SLICE: Duration = Duration::from_millis(250);

pub fn emit_tracker_events(app: &AppHandle, events: Vec<TrackerEvent>) {
    for event in events {
        let result = match event {
            TrackerEvent::Started(a) => app.emit("session-started", a),
            TrackerEvent::Updated(a) => app.emit("session-updated", a),
            TrackerEvent::Ended(s) => app.emit("session-ended", s),
            TrackerEvent::StatusChanged(s) => app.emit("tracker-status-changed", s),
            TrackerEvent::LimitReached(s) => {
                notify_limit_reached(app, &s);
                app.emit("limit-reached", s)
            }
            TrackerEvent::Reminder(r) => {
                notify_reminder(app, &r);
                app.emit("reminder", r)
            }
        };
        if let Err(e) = result {
            tracing::warn!(error = %e, "failed to emit tracker event");
        }
    }
}

/// "2h 05m" or "45m".
fn format_duration(secs: i64) -> String {
    let minutes = secs.max(0) / 60;
    if minutes >= 60 {
        format!("{}h {:02}m", minutes / 60, minutes % 60)
    } else {
        format!("{minutes}m")
    }
}

fn show_notification(app: &AppHandle, title: String, body: String) {
    if !app.state::<AppState>().settings().notifications_enabled {
        return;
    }
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        tracing::warn!(error = %e, "failed to show notification");
    }
}

fn notify_limit_reached(app: &AppHandle, status: &LimitStatus) {
    let title = match (status.limit.scope, &status.limit.game_name) {
        (LimitScope::Game, Some(name)) => format!("{name}: daily limit reached"),
        _ => "Daily play limit reached".to_string(),
    };
    let body = format!(
        "{} played today (limit {}).",
        format_duration(status.used_today_secs),
        format_duration(status.limit.daily_secs)
    );
    show_notification(app, title, body);
}

fn notify_reminder(app: &AppHandle, reminder: &Reminder) {
    show_notification(
        app,
        format!("Still playing {}", reminder.game_name),
        format!(
            "You have been playing for {}.",
            format_duration(reminder.elapsed_secs)
        ),
    );
}

pub fn emit_library_changed(app: &AppHandle) {
    if let Err(e) = app.emit("library-changed", ()) {
        tracing::warn!(error = %e, "failed to emit library-changed");
    }
}

pub fn emit_sessions_changed(app: &AppHandle) {
    if let Err(e) = app.emit("sessions-changed", ()) {
        tracing::warn!(error = %e, "failed to emit sessions-changed");
    }
}

/// Starts the tracker on its own thread. It does not depend on the webview, so it keeps
/// polling while the window is hidden.
pub fn spawn_tracker(app: AppHandle) {
    let spawned = std::thread::Builder::new()
        .name("tracker".into())
        .spawn(move || run_loop(app));
    if let Err(e) = spawned {
        tracing::error!(error = %e, "failed to start tracker thread");
    }
}

/// Imports every available launcher once, on its own thread, when `importOnStartup` is on.
/// Each import emits `library-changed` itself.
pub fn spawn_startup_import(app: AppHandle) {
    if !app.state::<AppState>().settings().import_on_startup {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("startup-import".into())
        .spawn(move || {
            let statuses = match crate::commands::imports::source_statuses(&app) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(error = %e, "startup import: cannot list launchers");
                    return;
                }
            };
            for status in statuses.into_iter().filter(|s| s.available) {
                match crate::commands::imports::run_import(&app, status.source) {
                    Ok(r) => tracing::info!(
                        source = r.source.as_str(),
                        discovered = r.discovered,
                        added = r.added,
                        updated = r.updated,
                        skipped = r.skipped,
                        errors = r.errors.len(),
                        "startup import finished"
                    ),
                    Err(e) => tracing::warn!(
                        source = status.source.as_str(),
                        error = %e,
                        "startup import failed"
                    ),
                }
            }
        });
    if let Err(e) = spawned {
        tracing::error!(error = %e, "failed to start import thread");
    }
}

fn run_loop(app: AppHandle) {
    let mut source = match SysinfoProcessSource::new() {
        Ok(source) => source,
        Err(e) => {
            tracing::error!(error = %e, "cannot enumerate processes; tracker not running");
            return;
        }
    };
    let clock = SystemClock;
    let mut idle = WinIdleSource::new();
    loop {
        let started = Instant::now();
        poll_once(&app, &mut source, &mut idle, &clock);
        // Re-read the interval while waiting so a settings change applies promptly.
        loop {
            let interval = app.state::<AppState>().settings().poll_interval_secs;
            let target = Duration::from_secs(u64::from(interval));
            let elapsed = started.elapsed();
            if elapsed >= target {
                break;
            }
            std::thread::sleep(SLEEP_SLICE.min(target - elapsed));
        }
    }
}

fn poll_once(
    app: &AppHandle,
    source: &mut dyn ProcessSource,
    idle: &mut dyn IdleSource,
    clock: &dyn Clock,
) {
    let state = app.state::<AppState>();
    let settings = state.settings();
    // Enumerating processes is the slow part; do it before taking any lock.
    let snapshot = settings.tracking_enabled.then(|| source.snapshot());
    // Idle (and gamepad) sampling only matters while a session runs. A session that starts on
    // this very poll gets no reading and counts as active input.
    let reading = if state.tracker().has_active() {
        idle.read()
    } else {
        idle.reset();
        IdleReading::default()
    };
    let now = clock.now();
    let result = {
        let mut tracker = state.tracker();
        let db = state.db();
        tracker.tick(&db, &settings, snapshot.as_ref(), reading, now)
    };
    match result {
        Ok(events) => emit_tracker_events(app, events),
        Err(e) => tracing::error!(error = %e, "tracker poll failed"),
    }
}
