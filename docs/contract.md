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
- Ids and counts are `i32` in Rust (specta cannot export `i64`); timestamps and durations are `i64`, both `number` in TypeScript.
- The generated `src/bindings.ts` exposes most structs as `Foo_Serialize` (what commands return) and `Foo_Deserialize` (what commands accept), with `Foo` as their union. The frontend's `src/api/types.ts` mirrors the `_Serialize` shapes and stays the frontend's source of truth; a compile-time check keeps the two aligned.
- Events are emitted with `app.emit` and are not part of the generated bindings; their names and payloads are only defined here.

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

| Command                  | Arguments                                                                  | Returns             | Notes                                                                                                                                                                                                                                                                                                                                                                                |
| ------------------------ | -------------------------------------------------------------------------- | ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `get_app_info`           |                                                                            | `AppInfo`           | exists                                                                                                                                                                                                                                                                                                                                                                               |
| `ping`                   |                                                                            | `string`            | exists                                                                                                                                                                                                                                                                                                                                                                               |
| `get_tracker_status`     |                                                                            | `TrackerStatus`     |                                                                                                                                                                                                                                                                                                                                                                                      |
| `set_tracking_enabled`   | `enabled: boolean`                                                         | `TrackerStatus`     | Also persists `trackingEnabled` in settings. Disabling stops polling; active auto sessions end immediately. Manual sessions keep running.                                                                                                                                                                                                                                            |
| `start_manual_session`   | `gameId: number`                                                           | `ActiveSession`     | Error if that game already has an active session.                                                                                                                                                                                                                                                                                                                                    |
| `stop_session`           | `sessionId: number`                                                        | `Session`           | Ends an active session now. Error if not active.                                                                                                                                                                                                                                                                                                                                     |
| `list_games`             | `includeHidden: boolean`                                                   | `GameSummary[]`     | Sorted by `sortName`.                                                                                                                                                                                                                                                                                                                                                                |
| `get_game`               | `gameId: number`                                                           | `GameSummary`       |                                                                                                                                                                                                                                                                                                                                                                                      |
| `add_game_from_exe`      | `exePath: string, name: string \| null`                                    | `Game`              | Source `manual`, `installDir` = parent directory, one `exact_path` executable. Name defaults to the file stem. Error if the exe is already attached to a game.                                                                                                                                                                                                                       |
| `update_game`            | `gameId: number, patch: GameUpdate`                                        | `Game`              | `sortName` is recomputed from `name` (lowercase, leading "the " dropped).                                                                                                                                                                                                                                                                                                            |
| `delete_game`            | `gameId: number`                                                           | `null`              | Cascades to executables and sessions. Error if the game has an active session.                                                                                                                                                                                                                                                                                                       |
| `list_game_executables`  | `gameId: number`                                                           | `GameExecutable[]`  |                                                                                                                                                                                                                                                                                                                                                                                      |
| `add_game_executable`    | `gameId: number, exePath: string, matchMode: MatchMode`                    | `GameExecutable`    | Path normalised by the backend.                                                                                                                                                                                                                                                                                                                                                      |
| `remove_game_executable` | `executableId: number`                                                     | `null`              |                                                                                                                                                                                                                                                                                                                                                                                      |
| `pick_executable`        |                                                                            | `string \| null`    | Native file dialog filtered to `*.exe`; `null` when cancelled. Uses tauri-plugin-dialog on the Rust side.                                                                                                                                                                                                                                                                            |
| `list_sessions`          | `query: SessionQuery`                                                      | `SessionWithGame[]` | Newest first. Active sessions included with `endedAt: null`.                                                                                                                                                                                                                                                                                                                         |
| `create_session`         | `gameId: number, startedAt: number, endedAt: number, note: string \| null` | `Session`           | Source `manual`. Error unless `endedAt > startedAt`.                                                                                                                                                                                                                                                                                                                                 |
| `update_session`         | `sessionId: number, patch: SessionUpdate`                                  | `Session`           | Recomputes `durationSecs`. Error if the session is active, or if the result would have `endedAt <= startedAt`.                                                                                                                                                                                                                                                                       |
| `delete_session`         | `sessionId: number`                                                        | `null`              | Error if the session is active.                                                                                                                                                                                                                                                                                                                                                      |
| `get_dashboard_stats`    |                                                                            | `DashboardStats`    |                                                                                                                                                                                                                                                                                                                                                                                      |
| `get_daily_totals`       | `days: number, gameId: number \| null`                                     | `DailyTotal[]`      | Today and the previous `days - 1` days.                                                                                                                                                                                                                                                                                                                                              |
| `get_game_totals`        | `from: number \| null, to: number \| null, limit: number`                  | `GameTotal[]`       | Descending by `totalSecs`, bounds on `startedAt`.                                                                                                                                                                                                                                                                                                                                    |
| `get_hourly_totals`      | `days: number`                                                             | `HourlyTotal[]`     |                                                                                                                                                                                                                                                                                                                                                                                      |
| `get_settings`           |                                                                            | `Settings`          |                                                                                                                                                                                                                                                                                                                                                                                      |
| `update_settings`        | `patch: SettingsUpdate`                                                    | `Settings`          | Tracker reads new values on its next poll. Validation errors for out-of-range values.                                                                                                                                                                                                                                                                                                |
| `seed_demo_data`         |                                                                            | `null`              | Inserts a deterministic demo dataset: 8 games with `source = "manual"` and `sourceId = "demo-1"` to `"demo-8"`, about 120 sessions spread over the last 60 days with varied hours and lengths from 10 minutes to 5 hours. No-op if the demo games already exist (any game with `sourceId` `"demo-1"` to `"demo-8"`). Emits `library-changed` and `sessions-changed` when it inserts. |

## Events

Names are kebab-case. Rust emits them with `app.emit(name, payload)`; the payload serialises like every other type (camelCase).

| Event                    | Payload         | Emitted when                                                                                                                                                                                             |
| ------------------------ | --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `session-started`        | `ActiveSession` | A session opens (auto match, reopen after a merge, or `start_manual_session`).                                                                                                                           |
| `session-updated`        | `ActiveSession` | Every poll, once per active session. Carries fresh `elapsedSecs`.                                                                                                                                        |
| `session-ended`          | `Session`       | A session closes (missed polls, `stop_session`, tracking disabled).                                                                                                                                      |
| `tracker-status-changed` | `TrackerStatus` | Tracking is enabled or disabled, or the set of active sessions changes.                                                                                                                                  |
| `library-changed`        | `null`          | Games or executables are added, edited or deleted (`add_game_from_exe`, `update_game`, `delete_game`, `add_game_executable`, `remove_game_executable`, `seed_demo_data`).                                |
| `sessions-changed`       | `null`          | Sessions are created, edited or deleted through commands (`create_session`, `update_session`, `delete_session`, `seed_demo_data`). Not emitted for tracker-driven changes; those use the session events. |

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

| Family   | Key                                                                                                                              |
| -------- | -------------------------------------------------------------------------------------------------------------------------------- |
| Tracker  | `["tracker", "status"]`                                                                                                          |
| Games    | `["games", "list", { includeHidden }]`, `["games", "detail", gameId]`, `["games", "executables", gameId]`                        |
| Sessions | `["sessions", "list", query]`                                                                                                    |
| Stats    | `["stats", "dashboard"]`, `["stats", "daily", days, gameId]`, `["stats", "games", from, to, limit]`, `["stats", "hourly", days]` |
| Settings | `["settings"]`, app info `["app", "info"]`                                                                                       |

### Invalidation

| Event                              | Invalidates                                                                      |
| ---------------------------------- | -------------------------------------------------------------------------------- |
| `session-started`, `session-ended` | `["tracker"]`, `["games"]`, `["sessions"]`, `["stats"]`                          |
| `session-updated`                  | Writes the payload into `["tracker", "status"]` with `setQueryData`; no refetch. |
| `tracker-status-changed`           | `["tracker"]`, `["settings"]`                                                    |
| `library-changed`                  | `["games"]`, `["sessions"]`, `["stats"]`                                         |
| `sessions-changed`                 | `["sessions"]`, `["games"]`, `["stats"]`                                         |

Mutations also invalidate their own family on success, so the UI stays correct if an event is missed.

## Phase 3: launcher imports

Additions to the contract above, same conventions.

### Types

```ts
export type ImportSource = "steam" | "epic" | "gog";

export interface ImportSourceStatus {
  source: ImportSource;
  /** The launcher was found on this machine */
  available: boolean;
  /** Install path when available, otherwise why not */
  detail: string | null;
  lastScanAt: number | null;
}

export interface DiscoveredGame {
  source: ImportSource;
  sourceId: string;
  name: string;
  installDir: string | null;
  /** Normalised exact executable paths known from the launcher; may be empty */
  executables: string[];
  /** Launcher-reported playtime, when the launcher tracks it */
  playtimeMinutes: number | null;
  /** A game with this source and sourceId already exists */
  alreadyImported: boolean;
}

export interface ImportResult {
  source: ImportSource;
  discovered: number;
  added: number;
  updated: number;
  skipped: number;
  errors: string[];
}

export interface SteamApiSettings {
  apiKey: string | null;
  steamId64: string | null;
}
```

`Settings` and `SettingsUpdate` gain `importOnStartup: boolean` (default `true`).

### Commands

| Command                  | Arguments                    | Returns                | Notes                                                                                                                                                                              |
| ------------------------ | ---------------------------- | ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `list_import_sources`    |                              | `ImportSourceStatus[]` | Always all three sources.                                                                                                                                                          |
| `scan_import_source`     | `source: ImportSource`       | `DiscoveredGame[]`     | Preview only, writes nothing.                                                                                                                                                      |
| `import_games`           | `source: ImportSource`       | `ImportResult`         | Scans and upserts. Emits `library-changed`.                                                                                                                                        |
| `get_steam_api_settings` |                              | `SteamApiSettings`     |                                                                                                                                                                                    |
| `set_steam_api_settings` | `settings: SteamApiSettings` | `null`                 | Stored locally in the settings store.                                                                                                                                              |
| `fetch_steam_playtime`   |                              | `ImportResult`         | Steam Web API `GetOwnedGames`; needs key, SteamID64 and a public game list. Updates `imported_playtime` for existing Steam games (`updated` counts them). Emits `library-changed`. |

### Import rules

- Upsert by `(source, sourceId)`. New games get `source`, `sourceId`, `name`, `sortName`, `installDir`. Existing games keep user-edited `name`, `hidden` and `afkDetectionEnabled`; only `installDir` and executables are refreshed.
- Executables: one `install_dir_prefix` rule for `installDir` when it exists, plus one `exact_path` rule per discovered executable. User-added rules are never removed.
- Launcher playtime is a baseline, stored in `imported_playtime (game_id, source)`, never as `sessions`. On every import or playtime fetch: `minutes = max(existing minutes, launcher minutes - tracked minutes for the game)`, so re-imports never double count hours we tracked ourselves and the baseline never shrinks. The baseline is part of every all-time figure (`GameSummary.totalSecs`, `DashboardStats.allTimeSecs`, all-time `topGames`, `get_game_totals` without bounds) and is excluded from anything with a date range (today, week, daily, hourly, bounded totals, sessions), because launchers report no dates. `GameSummary.importedMinutes` reports the baseline so the UI can say how much of the total came from launchers.
- Steam: install path from `HKCU\Software\Valve\Steam\SteamPath` only if it contains `steamapps\libraryfolders.vdf`, otherwise `HKLM\SOFTWARE\WOW6432Node\Valve\Steam\InstallPath`. Libraries from `libraryfolders.vdf`; games from `appmanifest_*.acf` (`appid`, `name`, `installdir`, `StateFlags`). Skip appid 228980 and manifests whose install directory does not exist. Cover art from `https://cdn.cloudflare.steamstatic.com/steam/apps/<appid>/header.jpg`, downloaded once to `<app data>/covers/steam/<appid>.jpg` and set as `coverPath`; download failures are not errors.
- Epic: `C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests\*.item` (fallback `AppDataPath` under `HKLM\SOFTWARE\WOW6432Node\Epic Games\EpicGamesLauncher`). Fields `DisplayName`, `InstallLocation`, `LaunchExecutable`, `AppName`, `MainGameAppName`, `bIsIncompleteInstall`. Skip incomplete installs and DLC (`AppName != MainGameAppName`). No playtime.
- GOG: registry `HKLM\SOFTWARE\WOW6432Node\GOG.com\Games\<id>` (`gameName`, `path`, `exe`) first. If `C:\ProgramData\GOG.com\Galaxy\storage\galaxy-2.0.db` exists, copy it to a temp file and read titles and `GameTimes.minutesInGame` read-only; a schema surprise becomes an error string and does not drop the registry games.
- Startup: when `importOnStartup` is on, run `import_games` for every available source on a background thread after the tracker starts.
- Tracker signal: while `steam.exe` is running, a non-zero `HKCU\Software\Valve\Steam\RunningAppID` matches the Steam game with that `sourceId`, even when its process path is unreadable.

### Frontend

- Settings page: an "Import" card listing the three sources with availability and last scan, a Scan button that previews discovered games in a dialog with an Import button, and result counts. A Steam Web API section with key and SteamID64 fields and a "Fetch playtime" button. The `importOnStartup` switch.
- Games page: an "Import" button opening the same dialog. Rows and detail show the cover when `coverPath` is set, via `convertFileSrc` from `@tauri-apps/api/core` in the Tauri client and a plain URL in the mock.
- Game detail: an "Imported playtime" line when `importedMinutes > 0`.
- Query keys: `["import", "sources"]`, `["import", "scan", source]`, `["steamApi", "settings"]`. `import_games` and `fetch_steam_playtime` invalidate `["games"]`, `["stats"]` and `["import"]`.

## Phase 4: AFK detection, limits and reminders

Additions to the contract above, same conventions.

### Types

```ts
export type LimitScope = "global" | "game";
/** notify: once per day when reached. notify_repeat: again every 15 minutes while still playing past it. */
export type LimitAction = "notify" | "notify_repeat";

export interface Limit {
  id: number;
  scope: LimitScope;
  /** null for global */
  gameId: number | null;
  gameName: string | null;
  dailySecs: number;
  action: LimitAction;
}

export interface LimitInput {
  scope: LimitScope;
  gameId: number | null;
  dailySecs: number;
  action: LimitAction;
}

export interface LimitStatus {
  limit: Limit;
  /** Today's play counted against this limit, active sessions included, AFK excluded */
  usedTodaySecs: number;
  /** max(0, dailySecs - usedTodaySecs) */
  remainingSecs: number;
  reached: boolean;
}

export interface Reminder {
  sessionId: number;
  gameId: number;
  gameName: string;
  elapsedSecs: number;
}
```

`Settings` and `SettingsUpdate` gain `notificationsEnabled: boolean` (default `true`) and `reminderIntervalSecs: number` (default `0`, meaning off; otherwise 300 to 14400).

### Commands

| Command        | Arguments                            | Returns         | Notes                                                                    |
| -------------- | ------------------------------------ | --------------- | ------------------------------------------------------------------------ |
| `list_limits`  |                                      | `LimitStatus[]` | Global first, then games by name.                                        |
| `create_limit` | `input: LimitInput`                  | `Limit`         | One global limit at most, one per game at most; `dailySecs` 60 to 86400. |
| `update_limit` | `limitId: number, input: LimitInput` | `Limit`         |                                                                          |
| `delete_limit` | `limitId: number`                    | `null`          |                                                                          |

### Events

| Event           | Payload       | Emitted when                                                                                                                |
| --------------- | ------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `limit-reached` | `LimitStatus` | A poll finds a limit newly reached today, and again every 15 minutes for `notify_repeat` while a counted session is active. |
| `reminder`      | `Reminder`    | A session's non-AFK elapsed time crosses each multiple of `reminderIntervalSecs`.                                           |

`session-updated` already carries `isAfk` and `afkSecs`; there is no separate AFK event. Limits and reminders also produce an OS notification through tauri-plugin-notification when `notificationsEnabled` is on.

### Idle rules

- Idle time comes from `GetLastInputInfo` each poll while any session is active. Gamepads do not reset it, so XInput slots 0 to 3 are polled too and a packet-number change counts as input. A locked workstation (`OpenInputDesktop` fails) counts as idle.
- A session becomes AFK when idle time exceeds `afkThresholdSecs`. AFK starts retroactively at the last input time, so the threshold minutes are not counted as play. It ends on the first poll with fresh input.
- `afkSecs` accumulates per poll while AFK; `durationSecs` and `elapsedSecs` exclude it. Games with `afkDetectionEnabled = false` never go AFK. Manual sessions follow the same rules.
- Limits count non-AFK time only. Day boundaries are local midnight, like the stats.

### Frontend

- Settings page: a "Limits" card listing each limit with a usage bar and remaining time, add, edit and delete through a dialog (scope, game, hours and minutes, action), plus the `notificationsEnabled` switch and a reminder interval select (off, 30 min, 1 h, 2 h, 3 h).
- Dashboard: a compact "Limits today" list when any limit exists, bar per limit, red when reached. The now-playing strip shows an AFK badge and the elapsed timer pauses while AFK.
- Toasts for `limit-reached` and `reminder` in the app, in addition to the OS notification.
- Query keys: `["limits", "list"]`. `limit-reached`, `session-started` and `session-ended` invalidate `["limits"]`; mutations invalidate `["limits"]`.

## Phase 5: library management, export and backup

Additions to the contract above, same conventions. The `tags` and `game_tags` tables from migration 1 are used; no new migration is needed.

### Types

```ts
export interface Tag {
  id: number;
  name: string;
}

export interface MergeResult {
  survivorId: number;
  mergedSessions: number;
  mergedExecutables: number;
}

export type ExportFormat = "csv" | "json";

export interface ExportOptions {
  format: ExportFormat;
  /** inclusive lower bound on startedAt */
  from: number | null;
  /** exclusive upper bound on startedAt */
  to: number | null;
  gameId: number | null;
}

export interface ExportResult {
  path: string;
  sessions: number;
  games: number;
}

export interface BackupInfo {
  path: string;
  createdAt: number;
  sizeBytes: number;
}
```

`GameSummary` gains `tags: Tag[]` (sorted by name).

### Commands

| Command            | Arguments                                    | Returns                | Notes                                                                                                                                                                                                                                                                                                              |
| ------------------ | -------------------------------------------- | ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `list_tags`        |                                              | `Tag[]`                | Sorted by name.                                                                                                                                                                                                                                                                                                    |
| `create_tag`       | `name: string`                               | `Tag`                  | Trimmed, 1 to 40 chars, unique case-insensitively.                                                                                                                                                                                                                                                                 |
| `rename_tag`       | `tagId: number, name: string`                | `Tag`                  | Same rules.                                                                                                                                                                                                                                                                                                        |
| `delete_tag`       | `tagId: number`                              | `null`                 | Cascades to `game_tags`.                                                                                                                                                                                                                                                                                           |
| `set_game_tags`    | `gameId: number, tagIds: number[]`           | `Tag[]`                | Replaces the game's tags.                                                                                                                                                                                                                                                                                          |
| `merge_games`      | `survivorId: number, mergedIds: number[]`    | `MergeResult`          | Moves sessions, executables (skipping duplicates), tags and `imported_playtime` (keeping the larger value per source) onto the survivor, then deletes the merged games. Error if any of them has an active session or if `survivorId` is in `mergedIds`.                                                           |
| `pick_image`       |                                              | `string \| null`       | Native file dialog for png, jpg, jpeg, webp; `null` when cancelled.                                                                                                                                                                                                                                                |
| `set_game_cover`   | `gameId: number, sourcePath: string \| null` | `Game`                 | Copies the file to `<app data>/covers/custom/<gameId>.<ext>` and sets `coverPath`; `null` clears `coverPath` and deletes the custom file.                                                                                                                                                                          |
| `export_data`      | `options: ExportOptions`                     | `ExportResult \| null` | Opens a save dialog (default name `gamelapse-sessions-<YYYYMMDD>.<ext>`); `null` when cancelled.                                                                                                                                                                                                                   |
| `backup_database`  |                                              | `BackupInfo \| null`   | Save dialog, default name `gamelapse-backup-<YYYYMMDD-HHMM>.db`; writes a consistent copy with `VACUUM INTO`; `null` when cancelled.                                                                                                                                                                               |
| `restore_database` |                                              | `null`                 | Open dialog filtered to `*.db`. Error if any session is active. Validates the file is SQLite with a `games` table, pauses the tracker, swaps the database file (keeping the previous one as `gamelapse.db.bak`), reopens, runs migrations and crash recovery, then emits `library-changed` and `sessions-changed`. |

Tag, merge and cover commands emit `library-changed`.

### Export formats

- CSV columns: `session_id, game, started_at, ended_at, duration_secs, afk_secs, source, note`, timestamps as local ISO 8601, UTF-8 with header, RFC 4180 quoting.
- JSON: `{ exportedAt, games: Game[], sessions: SessionWithGame[] }` with the same filters applied; `games` holds only games that appear in the sessions.

### Frontend

- Games page: tag chips above the list that filter it (multi-select, OR), tag badges on rows.
- Game detail: a tag editor (toggle existing tags, create a new one inline), a cover section with "Choose image" and "Clear" using `pick_image` and `set_game_cover`, and a "Merge games" dialog that picks other games to merge into this one, with a confirmation listing what moves.
- Settings page: a "Data" card with export (format, period preset, optional game, Export button, toast with the path), "Back up now", and "Restore from backup" behind a destructive confirmation that explains the current data is replaced and kept as a `.bak` file. After a restore, invalidate every query.
- Query keys: `["tags", "list"]`. Tag mutations invalidate `["tags"]` and `["games"]`; merge and cover mutations invalidate `["games"]`, `["sessions"]`, `["stats"]`, `["limits"]`.

## Phase 6: autostart and updates

Additions to the contract above, same conventions.

### Types

```ts
export interface UpdateInfo {
  version: string;
  currentVersion: string;
  /** Release date as an ISO string, when the feed provides it */
  date: string | null;
  notes: string | null;
}

export interface UpdateProgress {
  downloaded: number;
  total: number | null;
}
```

`Settings` and `SettingsUpdate` gain `startupHintDismissed: boolean` (default `false`).

### Commands

| Command            | Arguments          | Returns              | Notes                                                                                                  |
| ------------------ | ------------------ | -------------------- | ------------------------------------------------------------------------------------------------------ |
| `get_autostart`    |                    | `boolean`            | Whether the Windows autostart entry exists.                                                            |
| `set_autostart`    | `enabled: boolean` | `boolean`            | Creates or removes the entry; the entry launches with `--minimized`.                                   |
| `check_for_update` |                    | `UpdateInfo \| null` | Queries the GitHub Releases feed; `null` when up to date. Errors on network failure.                   |
| `install_update`   |                    | `null`               | Downloads, verifies the signature, installs and relaunches. Emits `update-progress` while downloading. |

### Events

| Event              | Payload          | Emitted when                                                                                             |
| ------------------ | ---------------- | -------------------------------------------------------------------------------------------------------- |
| `update-available` | `UpdateInfo`     | The silent check that runs about 10 seconds after launch finds a newer version. At most once per launch. |
| `update-progress`  | `UpdateProgress` | During the download started by `install_update`.                                                         |

### Frontend

- Settings page: a "General" card with a "Start with Windows" switch backed by `get_autostart` and `set_autostart`. An "Updates" card with the current version, the last check result, a "Check for updates" button, and, when an update exists, its version and notes with an "Install and restart" button and a progress bar fed by `update-progress`.
- Dashboard: a dismissible banner when `startupHintDismissed` is false and autostart is off, offering "Enable" (calls `set_autostart(true)`) and "Not now"; both set `startupHintDismissed` to true.
- Toast on `update-available` linking to Settings.
- Query keys: `["autostart"]`, `["update", "check"]`. `set_autostart` invalidates `["autostart"]`; `check_for_update` results are cached until the next explicit check.

## Phase 7: Heroic import, folder scan, metadata lookup

Additions to the contract above, same conventions. Principle: Gamelapse never scans or watches folders on its own. A scan runs only when the user clicks Scan, and nothing enters the library until the user confirms it.

### Types

```ts
// GameSource gains "amazon". ImportSource gains "heroic".

export interface ScanRoot {
  id: number;
  path: string;
  createdAt: number;
}

export interface ExeCandidate {
  path: string;
  sizeBytes: number;
  /** Higher is more likely the main game executable */
  score: number;
  /** Short human-readable reasons, e.g. "name matches folder", "Unreal shipping exe" */
  reasons: string[];
}

export interface ScanCandidate {
  rootId: number;
  /** Absolute path of the top-level folder under the root */
  folder: string;
  /** Cleaned from the folder name */
  suggestedName: string;
  /** Sorted by score, descending; junk executables excluded; at most 8 */
  executables: ExeCandidate[];
  confidence: "high" | "medium" | "low";
  /** A library game already has an executable rule under this folder */
  existingGameId: number | null;
}

export interface ScanSelection {
  folder: string;
  name: string;
  exePath: string;
  /** Optional cover to download, from a metadata match */
  coverUrl: string | null;
}

export interface MetadataMatch {
  steamAppId: number;
  name: string;
  coverUrl: string;
}
```

### Commands

| Command                | Arguments                                                   | Returns           | Notes                                                                                                                                                                                                                                       |
| ---------------------- | ----------------------------------------------------------- | ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `pick_folder`          |                                                             | `string \| null`  | Native folder dialog.                                                                                                                                                                                                                       |
| `list_scan_roots`      |                                                             | `ScanRoot[]`      | Sorted by path.                                                                                                                                                                                                                             |
| `add_scan_root`        | `path: string`                                              | `ScanRoot`        | Error if not a directory, already added, or inside or containing an existing root.                                                                                                                                                          |
| `remove_scan_root`     | `rootId: number`                                            | `null`            | Removing a root never touches games.                                                                                                                                                                                                        |
| `scan_folders`         | `rootIds: number[] \| null`                                 | `ScanCandidate[]` | `null` means all roots. Walks at most 5 levels deep, skips reparse points, groups by top-level folder. Runs only from this command.                                                                                                         |
| `add_scanned_games`    | `selection: ScanSelection[]`                                | `ImportResult`    | Creates `manual` games with `installDir` = folder, an `install_dir_prefix` rule for the folder and an `exact_path` rule for `exePath`; downloads `coverUrl` to `<app data>/covers/lookup/<gameId>.jpg` when given. Emits `library-changed`. |
| `lookup_game_metadata` | `name: string`                                              | `MetadataMatch[]` | Steam store search, at most 5 matches. Error on network failure. Called only from the scan dialog (one candidate at a time, at least 300 ms apart) and the Find cover button.                                                               |
| `apply_game_metadata`  | `gameId: number, match: MetadataMatch, renameGame: boolean` | `Game`            | Downloads the cover to `covers/lookup/<gameId>.jpg`, sets `coverPath`, renames when asked. Emits `library-changed`.                                                                                                                         |

### Heroic import rules

- Config dir: `%APPDATA%\heroic`. Available when it exists and holds at least one of the files below.
- Epic through legendary: `legendaryConfig\legendary\installed.json`, a map of app name to `{ title, install_path, executable, app_name }`. Game `source = "epic"`, `sourceId = app_name` (the same id the official Epic importer uses, so a game seen by both appears once). Executable = `install_path\executable` when `executable` is non-empty. Cover: `legendaryConfig\legendary\metadata\<app_name>.json`, `keyImages` entry of type `DieselGameBoxTall`, else `DieselGameBox`, downloaded to `covers/epic/<app_name>.jpg`.
- GOG through Heroic: `gog_store\installed.json` (`installed[]` with `appName`, `install_path`, `platform`) with titles and `art_cover` from `gog_store\library.json`. Only `platform = "windows"`. Game `source = "gog"`, `sourceId = appName`.
- Amazon through nile: `nile_config\nile\installed.json`; game `source = "amazon"`. Titles from `nile_config\nile\library.json` when present.
- Sideloaded: `sideload_apps\library.json`; game `source = "manual"`, `sourceId = "heroic-sideload:<app_name>"`, executable from the entry.
- Skip entries whose install folder does not exist. Playtime: import minutes when Heroic stores them (check the real files on this PC); otherwise none.
- Entries with DLC or add-on markers (`is_dlc`, an app name that differs from a `MainGameAppName`-style field) are skipped.

### Scan rules

- Executables are collected per top-level folder. Junk is excluded by file name patterns, case-insensitive: `unins*`, `setup*`, `install*`, `*redist*`, `vc_redist*`, `dxsetup*`, `dotnet*`, `*crash*`, `*handler*`, `*report*`, `UnrealCEFSubProcess*`, `*EasyAntiCheat*`, `BEService*`, `*uninstall*`, `python*`, `java*`, `node*`, files under `redist`, `_CommonRedist`, `DirectX`, `vcredist`, `dotnet`, `EasyAntiCheat` or `__Installer` folders, and files smaller than 512 KB.
- Score, additive: name matches the cleaned folder name +40; ends with `-Win64-Shipping.exe` +35; sits in the folder root or in `bin`, `Binaries\Win64`, `x64` +15; largest executable in the folder +20; name contains `launcher` -15; name contains `server`, `editor`, `benchmark`, `tool`, `config` -25.
- Confidence: high when the top score is at least 60 and leads the runner-up by 20 or more; medium when the top score is at least 30; otherwise low.
- Name cleanup: replace `-`, `_` and `.` with spaces; drop bracketed tags, version numbers (`v1.2`, `1.02`, `Build 123`), years in parentheses, and words `repack`, `fitgirl`, `dodi`, `elamigos`, `ankergames`, `goldberg`, `multi12` style tags; collapse spaces; title case.
- `existingGameId` is set when any library executable rule lies under the folder.

### Frontend

- Settings page: the Import card gains a Heroic row. A new "Games folders" card lists the roots with Add folder (`pick_folder` then `add_scan_root`), Remove, and a "Scan folders" button, with the line "Gamelapse scans only when you click Scan and adds only what you confirm."
- Scan dialog: progress while scanning, then one row per candidate: checkbox (checked by default for high confidence without `existingGameId`), editable name, executable select (score and reasons as a tooltip), a "Match" column that runs `lookup_game_metadata` for the visible candidates one at a time and shows the best match's name and thumbnail with a "Use" action that fills the name and sets `coverUrl`, and an "Add selected" button that calls `add_scanned_games` and shows the result. Candidates with `existingGameId` show an "In library" badge.
- Games page: a "Scan folders" button next to Import. With no roots, it opens Settings to the Games folders card.
- Game detail: a "Find cover" button opening a lookup dialog (search box prefilled with the name, matches with thumbnails, "Use cover" and "Use cover and name").
- Query keys: `["scanRoots", "list"]`, `["scan", "candidates"]`, `["metadata", name]`. Root mutations invalidate `["scanRoots"]`; `add_scanned_games` and `apply_game_metadata` invalidate `["games"]`, `["stats"]`, `["import"]`.
