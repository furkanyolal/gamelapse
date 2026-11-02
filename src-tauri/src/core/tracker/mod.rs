//! Process-detection and session-tracking state machine.
//! No Tauri types: the loop and event emission live in `crate::runtime`.
//!
//! Lock order for callers that share a [`Tracker`] with a database: tracker first, then db.

pub mod sysinfo_source;

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;

use crate::core::db::repo;
use crate::core::types::{
    ActiveSession, ActiveSource, GameExecutable, MatchMode, Session, SessionSource, Settings,
    TrackerStatus,
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
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    session_id: i32,
    source: ActiveSource,
    misses: u8,
}

#[derive(Default)]
pub struct Tracker {
    /// Active sessions by game id.
    active: HashMap<i32, Entry>,
    /// Games the user stopped by hand while their process is still running. They stay
    /// ignored until a poll no longer sees the process.
    suppressed: HashSet<i32>,
    last_poll_at: Option<i64>,
}

impl Tracker {
    pub fn new() -> Self {
        Self::default()
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
            is_afk: false,
            source: entry.source,
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
            Entry {
                session_id: session.id,
                source: ActiveSource::Manual,
                misses: 0,
            },
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
    ) -> AppResult<Vec<TrackerEvent>> {
        let snapshot = settings.tracking_enabled.then(|| source.snapshot());
        self.tick(conn, settings, snapshot.as_ref(), clock.now())
    }

    /// One poll. `snapshot` is `None` while tracking is disabled; manual sessions still tick.
    pub fn tick(
        &mut self,
        conn: &Connection,
        settings: &Settings,
        snapshot: Option<&Snapshot>,
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
            let session_id = match repo::last_ended_auto_session(conn, game_id)? {
                Some(prev)
                    if prev
                        .ended_at
                        .is_some_and(|e| e + settings.session_merge_gap_secs >= now) =>
                {
                    repo::reopen_session(conn, prev.id, now)?;
                    prev.id
                }
                _ => {
                    let s =
                        repo::insert_session(conn, game_id, now, None, SessionSource::Auto, None)?;
                    repo::write_heartbeat(conn, s.id, now)?;
                    s.id
                }
            };
            self.active.insert(
                game_id,
                Entry {
                    session_id,
                    source: ActiveSource::Auto,
                    misses: 0,
                },
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

        let changed = !started.is_empty() || !ended.is_empty();
        let mut events = Vec::new();
        for game_id in started {
            events.push(TrackerEvent::Started(self.active_view(conn, game_id, now)?));
        }
        events.extend(ended);
        for game_id in self.sorted_game_ids() {
            events.push(TrackerEvent::Updated(self.active_view(conn, game_id, now)?));
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
