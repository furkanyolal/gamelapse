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
- Launcher playtime goes to `imported_playtime (game_id, source)`, never to `sessions`.
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
