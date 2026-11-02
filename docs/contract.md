# IPC contract: Phase 1 (tracker) and Phase 2 (dashboard)

Single source of truth for the Tauri commands, events and payload types shared by the Rust backend and the React frontend. Both sides implement this exactly. A change to this file comes first; code follows.

## Conventions

- Command names are the snake_case Rust function names. The frontend invokes them as `invoke("list_games", { includeHidden })`. Argument keys in the payload are camelCase (Tauri default).
- Every struct serialises with `#[serde(rename_all = "camelCase")]`. Enums serialise as the exact strings shown below.
- Timestamps are UTC unix seconds (`number`). Durations are seconds (`number`). Daily buckets use the local date as `"YYYY-MM-DD"`.
- Commands return `Result<T, AppError>`; the frontend receives the error as a string message.
- Patch objects use `T | null` for every field. `null` means keep the current value. A nullable text field (for example a note) is cleared by sending an empty string.
- Sessions that span local midnight are split across days for daily totals. Sessions that span an hour boundary are split across hours for hourly totals.
- "Week" means the last 7 local days including today.

## Types

```ts
export type GameSource = "manual" | "steam" | "epic" | "gog";
export type SessionSource = "auto" | "manual" | "imported";
export type MatchMode = "exact_path" | "install_dir_prefix" | "exe_name";
export type Theme = "dark" | "light" | "system";

export interface AppInfo {
  version: string;
  dataDir: string;
  dbPath: string;
  logDir: string;
}

export interface Game {
  id: number;
  name: string;
  sortName: string;
  source: GameSource;
  sourceId: string | null;
  installDir: string | null;
  iconPath: string | null;
  coverPath: string | null;
  hidden: boolean;
  afkDetectionEnabled: boolean;
  createdAt: number;
}

export interface GameExecutable {
  id: number;
  gameId: number;
  /** Lowercase, backslashes, full path. For install_dir_prefix it is the directory, with no trailing slash. For exe_name it is the bare file name. */
  exePath: string;
  matchMode: MatchMode;
}

export interface Session {
  id: number;
  gameId: number;
  startedAt: number;
  /** null while the session is active */
  endedAt: number | null;
  /** (endedAt or now) - startedAt - afkSecs, never negative */
  durationSecs: number;
  afkSecs: number;
  source: SessionSource;
  note: string | null;
}

export interface SessionWithGame extends Session {
  gameName: string;
}

export interface ActiveSession {
  sessionId: number;
  gameId: number;
  gameName: string;
  startedAt: number;
  /** now - startedAt - afkSecs */
  elapsedSecs: number;
  afkSecs: number;
  /** Always false until Phase 4 */
  isAfk: boolean;
  source: "auto" | "manual";
}

export interface TrackerStatus {
  trackingEnabled: boolean;
  /** One entry per game currently being played; usually zero or one */
  active: ActiveSession[];
  lastPollAt: number | null;
}

export interface GameSummary {
  game: Game;
  totalSecs: number;
  todaySecs: number;
  weekSecs: number;
  sessionCount: number;
  /** Max startedAt over the game's sessions */
  lastPlayedAt: number | null;
  /** Sum of imported_playtime.minutes; 0 until Phase 3 */
  importedMinutes: number;
}

export interface GameTotal {
  gameId: number;
  gameName: string;
  totalSecs: number;
}

export interface DashboardStats {
  todaySecs: number;
  weekSecs: number;
  allTimeSecs: number;
  /** Games with at least one session */
  gamesPlayedCount: number;
  /** Top 5 games all time, descending */
  topGames: GameTotal[];
  /** Last 10 sessions, newest first, active sessions included */
  recentSessions: SessionWithGame[];
}

/** One entry per day in the range, oldest first, zero-filled */
export interface DailyTotal {
  date: string;
  totalSecs: number;
}

/** 24 entries, hour 0..23 local */
export interface HourlyTotal {
  hour: number;
  totalSecs: number;
}

export interface Settings {
  /** default 5, min 1, max 60 */
  pollIntervalSecs: number;
  /** default 600 */
  afkThresholdSecs: number;
  /** default 120 */
  sessionMergeGapSecs: number;
  /** default false */
  requireForeground: boolean;
  /** default true */
  trackingEnabled: boolean;
  /** default "dark" */
  theme: Theme;
}

export interface SettingsUpdate {
  pollIntervalSecs: number | null;
  afkThresholdSecs: number | null;
  sessionMergeGapSecs: number | null;
  requireForeground: boolean | null;
  trackingEnabled: boolean | null;
  theme: Theme | null;
}

export interface GameUpdate {
  name: string | null;
  hidden: boolean | null;
  afkDetectionEnabled: boolean | null;
}

export interface SessionUpdate {
  startedAt: number | null;
  endedAt: number | null;
  /** null keeps, "" clears */
  note: string | null;
}

export interface SessionQuery {
  gameId: number | null;
  /** inclusive lower bound on startedAt */
  from: number | null;
  /** exclusive upper bound on startedAt */
  to: number | null;
  limit: number;
  offset: number;
}
```

## Commands

| Command | Arguments | Returns | Notes |
|---|---|---|---|
| `get_app_info` | | `AppInfo` | exists |
| `ping` | | `string` | exists |
| `get_tracker_status` | | `TrackerStatus` | |
| `set_tracking_enabled` | `enabled: boolean` | `TrackerStatus` | Also persists `trackingEnabled` in settings. Disabling stops polling; active auto sessions end immediately. Manual sessions keep running. |
| `start_manual_session` | `gameId: number` | `ActiveSession` | Error if that game already has an active session. |
| `stop_session` | `sessionId: number` | `Session` | Ends an active session now. Error if not active. |
| `list_games` | `includeHidden: boolean` | `GameSummary[]` | Sorted by `sortName`. |
| `get_game` | `gameId: number` | `GameSummary` | |
| `add_game_from_exe` | `exePath: string, name: string \| null` | `Game` | Source `manual`, `installDir` = parent directory, one `exact_path` executable. Name defaults to the file stem. Error if the exe is already attached to a game. |
| `update_game` | `gameId: number, patch: GameUpdate` | `Game` | `sortName` is recomputed from `name` (lowercase, leading "the " dropped). |
| `delete_game` | `gameId: number` | `null` | Cascades to executables and sessions. Error if the game has an active session. |
| `list_game_executables` | `gameId: number` | `GameExecutable[]` | |
| `add_game_executable` | `gameId: number, exePath: string, matchMode: MatchMode` | `GameExecutable` | Path normalised by the backend. |
| `remove_game_executable` | `executableId: number` | `null` | |
| `pick_executable` | | `string \| null` | Native file dialog filtered to `*.exe`; `null` when cancelled. Uses tauri-plugin-dialog on the Rust side. |
| `list_sessions` | `query: SessionQuery` | `SessionWithGame[]` | Newest first. Active sessions included with `endedAt: null`. |
| `create_session` | `gameId: number, startedAt: number, endedAt: number, note: string \| null` | `Session` | Source `manual`. Error unless `endedAt > startedAt`. |
| `update_session` | `sessionId: number, patch: SessionUpdate` | `Session` | Recomputes `durationSecs`. Error if the session is active, or if the result would have `endedAt <= startedAt`. |
| `delete_session` | `sessionId: number` | `null` | Error if the session is active. |
| `get_dashboard_stats` | | `DashboardStats` | |
| `get_daily_totals` | `days: number, gameId: number \| null` | `DailyTotal[]` | Today and the previous `days - 1` days. |
| `get_game_totals` | `from: number \| null, to: number \| null, limit: number` | `GameTotal[]` | Descending by `totalSecs`, bounds on `startedAt`. |
| `get_hourly_totals` | `days: number` | `HourlyTotal[]` | |
| `get_settings` | | `Settings` | |
| `update_settings` | `patch: SettingsUpdate` | `Settings` | Tracker reads new values on its next poll. Validation errors for out-of-range values. |
| `seed_demo_data` | | `null` | Inserts a deterministic demo dataset: 8 games with `source = "manual"` and `sourceId = "demo-1"` to `"demo-8"`, about 120 sessions spread over the last 60 days with varied hours and lengths from 10 minutes to 5 hours. No-op if the demo games already exist (any game with `sourceId` `"demo-1"` to `"demo-8"`). Emits `library-changed` and `sessions-changed` when it inserts.

## Events

Names are kebab-case. Rust emits them with `app.emit(name, payload)`; the payload serialises like every other type (camelCase).

| Event | Payload | Emitted when |
|---|---|---|
| `session-started` | `ActiveSession` | A session opens (auto match, reopen after a merge, or `start_manual_session`). |
| `session-updated` | `ActiveSession` | Every poll, once per active session. Carries fresh `elapsedSecs`. |
| `session-ended` | `Session` | A session closes (missed polls, `stop_session`, tracking disabled). |
| `tracker-status-changed` | `TrackerStatus` | Tracking is enabled or disabled, or the set of active sessions changes. |
| `library-changed` | `null` | Games or executables are added, edited or deleted (`add_game_from_exe`, `update_game`, `delete_game`, `add_game_executable`, `remove_game_executable`, `seed_demo_data`). |
| `sessions-changed` | `null` | Sessions are created, edited or deleted through commands (`create_session`, `update_session`, `delete_session`, `seed_demo_data`). Not emitted for tracker-driven changes; those use the session events. |

## Tracker behaviour

Rules the Rust tracker follows. Logic lives in `core/tracker/` with no Tauri types.

- **Polling:** one background task, interval `pollIntervalSecs` from settings, re-read on every poll.
- **Enumeration:** `sysinfo` lists processes with their exe paths. Paths are normalised to lowercase with backslashes, the same form as `GameExecutable.exePath`. Processes without a readable path are skipped.
- **Matching:** a process matches a game if any of its executables matches by mode:
  - `exact_path`: the process path equals `exePath`.
  - `install_dir_prefix`: the process path starts with `exePath` followed by `\`.
  - `exe_name`: the file name of the process path equals `exePath`.
- **Start:** a session starts on the first poll that matches. A game with several matching processes still has one session.
- **Heartbeat:** every poll writes a `session_heartbeats` row for each active session.
- **End:** an auto session ends after two consecutive polls without a match, with `ended_at` = the last heartbeat.
- **Merge:** if the same game's previous session ended within `sessionMergeGapSecs` of the new match, reopen it (`endedAt` back to null) instead of creating a new one.
- **Crash recovery:** at startup, close any session still open at its last heartbeat time (or `startedAt` if it has none).
- **Manual sessions:** tick independently of process matching, and block an auto session for the same game while they run.
- **Disabling tracking:** `set_tracking_enabled(false)` ends auto sessions immediately and stops polling. Manual sessions keep running.
- **Foreground:** with `requireForeground`, a game only counts as matched on a poll when the foreground window's process matches it.
- **Testability:** core logic sits behind `ProcessSource` (process and foreground snapshots) and `Clock` traits, so unit tests feed fake snapshots and a fake clock.

## Frontend data layer

- `src/api/types.ts` mirrors the Types section by hand until the generated `src/bindings.ts` replaces it at integration.
- `src/api/` defines an `Api` interface with one camelCase method per command (`listGames(includeHidden)`, `startManualSession(gameId)`, ...) and `on(event, handler)`, which returns an unsubscribe function. Event names and payloads are typed from the Events table.
- `src/api/tauri.ts` implements `Api` with `invoke` and `listen` from `@tauri-apps/api`.
- `src/api/mock.ts` is a deterministic in-memory fake: seeded data, working mutations, and a fake active session that ticks and emits events. It is used automatically when `window.__TAURI_INTERNALS__` is absent, so the UI runs in a plain browser.
- `src/api/index.ts` exports the chosen `api` (tauri or mock).

### Query keys

Hooks in `src/hooks/` use TanStack Query with these keys; arguments go last.

| Family | Key |
|---|---|
| Tracker | `["tracker", "status"]` |
| Games | `["games", "list", { includeHidden }]`, `["games", "detail", gameId]`, `["games", "executables", gameId]` |
| Sessions | `["sessions", "list", query]` |
| Stats | `["stats", "dashboard"]`, `["stats", "daily", days, gameId]`, `["stats", "games", from, to, limit]`, `["stats", "hourly", days]` |
| Settings | `["settings"]`, app info `["app", "info"]` |

### Invalidation

| Event | Invalidates |
|---|---|
| `session-started`, `session-ended` | `["tracker"]`, `["games"]`, `["sessions"]`, `["stats"]` |
| `session-updated` | Writes the payload into `["tracker", "status"]` with `setQueryData`; no refetch. |
| `tracker-status-changed` | `["tracker"]`, `["settings"]` |
| `library-changed` | `["games"]`, `["sessions"]`, `["stats"]` |
| `sessions-changed` | `["sessions"]`, `["games"]`, `["stats"]` |

Mutations also invalidate their own family on success, so the UI stays correct if an event is missed.
