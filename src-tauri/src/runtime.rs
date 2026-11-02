//! Glue between the tracker core and Tauri: the polling thread and event emission.

use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};

use crate::core::tracker::sysinfo_source::SysinfoProcessSource;
use crate::core::tracker::{Clock, ProcessSource, SystemClock, TrackerEvent};
use crate::state::AppState;

const SLEEP_SLICE: Duration = Duration::from_millis(250);

pub fn emit_tracker_events(app: &AppHandle, events: Vec<TrackerEvent>) {
    for event in events {
        let result = match event {
            TrackerEvent::Started(a) => app.emit("session-started", a),
            TrackerEvent::Updated(a) => app.emit("session-updated", a),
            TrackerEvent::Ended(s) => app.emit("session-ended", s),
            TrackerEvent::StatusChanged(s) => app.emit("tracker-status-changed", s),
        };
        if let Err(e) = result {
            tracing::warn!(error = %e, "failed to emit tracker event");
        }
    }
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

fn run_loop(app: AppHandle) {
    let mut source = match SysinfoProcessSource::new() {
        Ok(source) => source,
        Err(e) => {
            tracing::error!(error = %e, "cannot enumerate processes; tracker not running");
            return;
        }
    };
    let clock = SystemClock;
    loop {
        let started = Instant::now();
        poll_once(&app, &mut source, &clock);
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

fn poll_once(app: &AppHandle, source: &mut dyn ProcessSource, clock: &dyn Clock) {
    let state = app.state::<AppState>();
    let settings = state.settings();
    // Enumerating processes is the slow part; do it before taking any lock.
    let snapshot = settings.tracking_enabled.then(|| source.snapshot());
    let now = clock.now();
    let result = {
        let mut tracker = state.tracker();
        let db = state.db();
        tracker.tick(&db, &settings, snapshot.as_ref(), now)
    };
    match result {
        Ok(events) => emit_tracker_events(app, events),
        Err(e) => tracing::error!(error = %e, "tracker poll failed"),
    }
}
