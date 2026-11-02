//! Process-detection and session-tracking state machine.
//! No Tauri types: the loop and event emission live in `crate::runtime`.
//!
//! Lock order for callers that share a [`Tracker`] with a database: tracker first, then db.

pub mod sysinfo_source;

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;

use crate::core::db::repo;
use crate::core::idle::IdleReading;
use crate::core::limits::LimitMonitor;
use crate::core::types::{
    ActiveSession, ActiveSource, GameExecutable, LimitStatus, MatchMode, Reminder, Session,
    SessionSource, Settings, TrackerStatus,
};
use crate::error::{AppError, AppResult};

/// Consecutive polls without a match before an auto session ends.
const MISSED_POLLS_TO_END: u8 = 2;

/// What is running right now. All paths are normalised (lowercase, backslashes).
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub processes: Vec<String>,
    pub foreground: Option<String>,
    /// `HKCU\Software\Valve\Steam\RunningAppID`, read only while `steam.exe` runs.
    /// `None` when Steam is not running or no app is running.
    pub steam_running_app_id: Option<u32>,
}

impl Snapshot {
    /// True if a process named `steam.exe` is in the snapshot.
    pub fn steam_running(&self) -> bool {
        self.processes
            .iter()
            .any(|p| p.rsplit('\\').next() == Some("steam.exe"))
    }
}

pub trait ProcessSource: Send {
    fn snapshot(&mut self) -> Snapshot;
}

pub trait Clock: Send + Sync {
    /// UTC unix seconds.
    fn now(&self) -> i64;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> i64 {
        chrono::Utc::now().timestamp()
    }
}

/// Whether a running process path matches an executable rule. Both sides are normalised.
pub fn matches(exe: &GameExecutable, process_path: &str) -> bool {
    match exe.match_mode {
        MatchMode::ExactPath => process_path == exe.exe_path,
        MatchMode::InstallDirPrefix => process_path
            .strip_prefix(exe.exe_path.as_str())
            .is_some_and(|rest| rest.starts_with('\\')),
        MatchMode::ExeName => process_path.rsplit('\\').next() == Some(exe.exe_path.as_str()),
    }
}

/// Games with at least one executable matching the snapshot. With `require_foreground`
/// only the foreground process counts.
///
/// `steam_game` is the Steam game whose app id equals `snapshot.steam_running_app_id`; it
/// counts as matched even when no process path is readable. Steam does not say which window
/// belongs to the app, so the signal is ignored under `require_foreground`.
pub fn matched_games(
    executables: &[GameExecutable],
    snapshot: &Snapshot,
    require_foreground: bool,
    steam_game: Option<i32>,
) -> HashSet<i32> {
    let foreground: Vec<&String> = snapshot.foreground.iter().collect();
    let candidates: Vec<&String> = if require_foreground {
        foreground
    } else {
        snapshot.processes.iter().chain(foreground).collect()
    };
    let mut matched: HashSet<i32> = executables
        .iter()
        .filter(|exe| candidates.iter().any(|p| matches(exe, p)))
        .map(|exe| exe.game_id)
        .collect();
    if !require_foreground && let Some(id) = steam_game {
        matched.insert(id);
    }
    matched
}

/// Closes every session left open by a crash at its last heartbeat (or `startedAt`).
pub fn recover_crashed_sessions(conn: &Connection) -> AppResult<usize> {
    repo::recover_open_sessions(conn)
}

#[derive(Clone, Debug)]
pub enum TrackerEvent {
    Started(ActiveSession),
    Updated(ActiveSession),
    Ended(Session),
    StatusChanged(TrackerStatus),
    LimitReached(LimitStatus),
    Reminder(Reminder),
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    session_id: i32,
    source: ActiveSource,
    misses: u8,
    /// Currently AFK.
    afk: bool,
    /// Cumulative AFK seconds; mirrors `sessions.afk_secs`.
    afk_secs: i64,
    /// Wall-clock time up to which AFK time has been accounted.
    afk_counted_to: i64,
    /// Non-AFK elapsed seconds when the entry was created (non-zero for a reopened session).
    start_elapsed: i64,
    /// Reminder interval and the last multiple of it already reminded (or skipped).
    reminded: Option<(u32, i64)>,
}

impl Entry {
    fn new(
        session_id: i32,
        source: ActiveSource,
        afk_secs: i64,
        start_elapsed: i64,
        now: i64,
    ) -> Self {
        Self {
            session_id,
            source,
            misses: 0,
            afk: false,
            afk_secs,
            afk_counted_to: now,
            start_elapsed,
            reminded: None,
        }
    }
}

#[derive(Default)]
pub struct Tracker {
    /// Active sessions by game id.
    active: HashMap<i32, Entry>,
    /// Games the user stopped by hand while their process is still running. They stay
    /// ignored until a poll no longer sees the process.
    suppressed: HashSet<i32>,
    last_poll_at: Option<i64>,
    limits: LimitMonitor,
}

impl Tracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// True while any session is open; the runtime samples idle time only then.
    pub fn has_active(&self) -> bool {
        !self.active.is_empty()
    }

    fn active_view(&self, conn: &Connection, game_id: i32, now: i64) -> AppResult<ActiveSession> {
        let entry = self.active[&game_id];
        let session = repo::get_session(conn, entry.session_id, now)?;
        let game = repo::get_game(conn, game_id)?;
        Ok(ActiveSession {
            session_id: session.id,
            game_id,
            game_name: game.name,
            started_at: session.started_at,
            elapsed_secs: session.duration_secs,
            afk_secs: session.afk_secs,
            is_afk: entry.afk,
            source: entry.source,
            hidden: game.hidden,
        })
    }

    /// Applies one idle reading to a session's AFK state and persists `afk_secs`.
    ///
    /// AFK begins when idle time exceeds the threshold (or the workstation is locked) and is
    /// back-dated to the last input, so the idle stretch is not counted as play. While AFK,
    /// each poll adds the time since the previous one. The first poll below the threshold ends it.
    fn update_afk(
        &mut self,
        conn: &Connection,
        game_id: i32,
        settings: &Settings,
        idle: IdleReading,
        now: i64,
    ) -> AppResult<()> {
        let mut entry = self.active[&game_id];
        let threshold = settings.afk_threshold_secs;
        // A threshold of 0 turns detection off.
        let enabled = threshold > 0 && repo::get_game(conn, game_id)?.afk_detection_enabled;
        let idle_over = idle.locked || idle.idle_secs.is_some_and(|i| i > threshold);
        if enabled && idle_over {
            let from = if entry.afk {
                entry.afk_counted_to
            } else {
                let started = repo::get_session(conn, entry.session_id, now)?.started_at;
                let last_input = now - idle.idle_secs.unwrap_or(0).max(0);
                entry.afk = true;
                last_input.max(started).max(entry.afk_counted_to)
            };
            entry.afk_secs += (now - from).max(0);
            entry.afk_counted_to = now;
        } else {
            // `afk_counted_to` stays put: any later AFK episode starts at an input newer than it.
            entry.afk = false;
        }
        repo::write_afk_secs(conn, entry.session_id, entry.afk_secs)?;
        self.active.insert(game_id, entry);
        Ok(())
    }

    /// A reminder when the session's non-AFK elapsed time passed a new multiple of `interval`.
    /// Only one reminder per poll, even if several multiples passed.
    fn check_reminder(&mut self, view: &ActiveSession, interval: u32) -> Option<Reminder> {
        let entry = self.active.get_mut(&view.game_id)?;
        if interval == 0 {
            entry.reminded = Some((0, 0));
            return None;
        }
        let multiple = view.elapsed_secs / i64::from(interval);
        let last = match entry.reminded {
            Some((iv, last)) if iv == interval => last,
            // Interval changed mid-session: start counting from where the session is now.
            Some(_) => multiple,
            None => entry.start_elapsed / i64::from(interval),
        };
        entry.reminded = Some((interval, last.max(multiple)));
        (multiple > last).then(|| Reminder {
            session_id: view.session_id,
            game_id: view.game_id,
            game_name: view.game_name.clone(),
            elapsed_secs: view.elapsed_secs,
        })
    }

    fn sorted_game_ids(&self) -> Vec<i32> {
        let mut ids: Vec<i32> = self.active.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    pub fn status(
        &self,
        conn: &Connection,
        tracking_enabled: bool,
        now: i64,
    ) -> AppResult<TrackerStatus> {
        let mut active = Vec::new();
        for id in self.sorted_game_ids() {
            active.push(self.active_view(conn, id, now)?);
        }
        Ok(TrackerStatus {
            tracking_enabled,
            active,
            last_poll_at: self.last_poll_at,
        })
    }

    /// Ends every active auto session at `now`.
    fn end_auto(&mut self, conn: &Connection, now: i64) -> AppResult<Vec<TrackerEvent>> {
        let mut events = Vec::new();
        for id in self.sorted_game_ids() {
            let entry = self.active[&id];
            if entry.source == ActiveSource::Auto {
                events.push(TrackerEvent::Ended(repo::end_session(
                    conn,
                    entry.session_id,
                    now,
                )?));
                self.active.remove(&id);
            }
        }
        Ok(events)
    }

    /// Reacts to tracking being switched on or off (command or settings patch).
    pub fn set_tracking_enabled(
        &mut self,
        conn: &Connection,
        enabled: bool,
        now: i64,
    ) -> AppResult<Vec<TrackerEvent>> {
        let mut events = if enabled {
            Vec::new()
        } else {
            self.end_auto(conn, now)?
        };
        events.push(TrackerEvent::StatusChanged(
            self.status(conn, enabled, now)?,
        ));
        Ok(events)
    }

    pub fn start_manual(
        &mut self,
        conn: &Connection,
        tracking_enabled: bool,
        game_id: i32,
        now: i64,
    ) -> AppResult<(ActiveSession, Vec<TrackerEvent>)> {
        repo::get_game(conn, game_id)?;
        if self.active.contains_key(&game_id)
            || repo::active_session_for_game(conn, game_id)?.is_some()
        {
            return Err(AppError::Other("game already has an active session".into()));
        }
        let session = repo::insert_session(conn, game_id, now, None, SessionSource::Manual, None)?;
        repo::write_heartbeat(conn, session.id, now)?;
        self.active.insert(
            game_id,
            Entry::new(session.id, ActiveSource::Manual, 0, 0, now),
        );
        let view = self.active_view(conn, game_id, now)?;
        let events = vec![
            TrackerEvent::Started(view.clone()),
            TrackerEvent::StatusChanged(self.status(conn, tracking_enabled, now)?),
        ];
        Ok((view, events))
    }

    pub fn stop_session(
        &mut self,
        conn: &Connection,
        tracking_enabled: bool,
        session_id: i32,
        now: i64,
    ) -> AppResult<(Session, Vec<TrackerEvent>)> {
        let current = repo::get_session(conn, session_id, now)?;
        if current.ended_at.is_some() {
            return Err(AppError::Other("session is not active".into()));
        }
        let ended = repo::end_session(conn, session_id, now)?;
        if let Some(entry) = self.active.remove(&current.game_id)
            && entry.source == ActiveSource::Auto
        {
            self.suppressed.insert(current.game_id);
        }
        let events = vec![
            TrackerEvent::Ended(ended.clone()),
            TrackerEvent::StatusChanged(self.status(conn, tracking_enabled, now)?),
        ];
        Ok((ended, events))
    }

    /// Takes a snapshot from `source` and runs one tick (the convenience used by tests).
    pub fn poll(
        &mut self,
        conn: &Connection,
        settings: &Settings,
        source: &mut dyn ProcessSource,
        clock: &dyn Clock,
        idle: IdleReading,
    ) -> AppResult<Vec<TrackerEvent>> {
        let snapshot = settings.tracking_enabled.then(|| source.snapshot());
        self.tick(conn, settings, snapshot.as_ref(), idle, clock.now())
    }

    /// One poll. `snapshot` is `None` while tracking is disabled; manual sessions still tick.
    /// `idle` is the idle sample for this poll; the default (unknown, unlocked) never goes AFK.
    pub fn tick(
        &mut self,
        conn: &Connection,
        settings: &Settings,
        snapshot: Option<&Snapshot>,
        idle: IdleReading,
        now: i64,
    ) -> AppResult<Vec<TrackerEvent>> {
        self.last_poll_at = Some(now);
        let mut started = Vec::new();
        let mut ended = Vec::new();

        let matched = match snapshot {
            Some(snap) if settings.tracking_enabled => {
                let executables = repo::list_all_executables(conn)?;
                let steam_game = match snap.steam_running_app_id {
                    Some(app_id) => repo::steam_game_for_app(conn, app_id)?,
                    None => None,
                };
                matched_games(&executables, snap, settings.require_foreground, steam_game)
            }
            _ => HashSet::new(),
        };
        self.suppressed.retain(|g| matched.contains(g));

        if !settings.tracking_enabled {
            ended.extend(self.end_auto(conn, now)?);
        }

        let mut to_start: Vec<i32> = matched
            .iter()
            .copied()
            .filter(|g| !self.suppressed.contains(g))
            .collect();
        to_start.sort_unstable();

        for game_id in to_start {
            if let Some(entry) = self.active.get_mut(&game_id) {
                if entry.source == ActiveSource::Auto {
                    entry.misses = 0;
                    repo::write_heartbeat(conn, entry.session_id, now)?;
                }
                // A manual session for the game blocks the auto one.
                continue;
            }
            let (session_id, afk_secs, elapsed) =
                match repo::last_ended_auto_session(conn, game_id)? {
                    Some(prev)
                        if prev
                            .ended_at
                            .is_some_and(|e| e + settings.session_merge_gap_secs >= now) =>
                    {
                        repo::reopen_session(conn, prev.id, now)?;
                        let reopened = repo::get_session(conn, prev.id, now)?;
                        (prev.id, reopened.afk_secs, reopened.duration_secs)
                    }
                    _ => {
                        let s = repo::insert_session(
                            conn,
                            game_id,
                            now,
                            None,
                            SessionSource::Auto,
                            None,
                        )?;
                        repo::write_heartbeat(conn, s.id, now)?;
                        (s.id, 0, 0)
                    }
                };
            self.active.insert(
                game_id,
                Entry::new(session_id, ActiveSource::Auto, afk_secs, elapsed, now),
            );
            started.push(game_id);
        }

        for game_id in self.sorted_game_ids() {
            let entry = self.active[&game_id];
            match entry.source {
                ActiveSource::Manual => repo::write_heartbeat(conn, entry.session_id, now)?,
                ActiveSource::Auto if !matched.contains(&game_id) => {
                    let misses = entry.misses + 1;
                    if misses >= MISSED_POLLS_TO_END {
                        let last = repo::last_heartbeat(conn, entry.session_id)?;
                        let at = last.unwrap_or(now);
                        ended.push(TrackerEvent::Ended(repo::end_session(
                            conn,
                            entry.session_id,
                            at,
                        )?));
                        self.active.remove(&game_id);
                    } else {
                        self.active.get_mut(&game_id).expect("entry exists").misses = misses;
                    }
                }
                ActiveSource::Auto => {}
            }
        }

        // Sessions that missed this poll got no heartbeat, so their AFK state stays as it was.
        for game_id in self.sorted_game_ids() {
            let entry = self.active[&game_id];
            if entry.source == ActiveSource::Manual || matched.contains(&game_id) {
                self.update_afk(conn, game_id, settings, idle, now)?;
            }
        }

        let changed = !started.is_empty() || !ended.is_empty();
        let mut events = Vec::new();
        for game_id in started {
            events.push(TrackerEvent::Started(self.active_view(conn, game_id, now)?));
        }
        events.extend(ended);
        let mut reminders = Vec::new();
        for game_id in self.sorted_game_ids() {
            let view = self.active_view(conn, game_id, now)?;
            let reminder = self.check_reminder(&view, settings.reminder_interval_secs);
            // The reminder state still advances for hidden games; only the emit is skipped.
            if !view.hidden {
                reminders.extend(reminder);
            }
            events.push(TrackerEvent::Updated(view));
        }
        events.extend(reminders.into_iter().map(TrackerEvent::Reminder));

        let counting: Vec<i32> = self
            .sorted_game_ids()
            .into_iter()
            .filter(|id| !self.active[id].afk)
            .collect();
        match self.limits.evaluate(conn, now, &counting) {
            Ok(due) => events.extend(due.into_iter().map(TrackerEvent::LimitReached)),
            Err(e) => tracing::warn!(error = %e, "limit evaluation failed"),
        }

        if changed {
            events.push(TrackerEvent::StatusChanged(self.status(
                conn,
                settings.tracking_enabled,
                now,
            )?));
        }
        Ok(events)
    }
}
