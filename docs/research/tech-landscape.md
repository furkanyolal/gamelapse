# Game Time Tracker: Tech Landscape (2026-11-02)

Versions marked ✔ were checked against crates.io/npm on 2026-11-02. Everything else (file formats, table names, API quirks) is from prior knowledge and **must be verified against a real install** before relying on it.

## 1. Tauri 2

- ✔ `tauri` 2.12.5, `@tauri-apps/cli` / `@tauri-apps/api` 2.12.x. Docs: https://v2.tauri.app
- Tray: enable the `tray-icon` cargo feature; use `tauri::tray::TrayIconBuilder` + `tauri::menu::{Menu, MenuItem}`; handle `on_menu_event` and `on_tray_icon_event` (left-click shows the window).
- Plugins (all ✔ latest): `tauri-plugin-autostart` 2.7.0, `-notification` 2.8.0, `-single-instance` 2.5.5, `-updater` 2.14.1, `-store` 2.5.0, `-window-state` 2.5.0, `-opener` 2.7.2 (replaces most of `-shell` for opening URLs/paths), `-shell` 2.6.0 (only if spawning sidecars), `-sql` 2.6.0.
- Single-instance must be registered **first**; on second launch, show/focus the existing window.
- Hidden-on-close structure: in `on_window_event`, on `CloseRequested` call `api.prevent_close()` + `window.hide()`. Quit only via tray menu (`app.exit(0)`). Run the tracker as a Rust background task (`tauri::async_runtime::spawn`), independent of the webview, so tracking works with the window hidden. Start with `--minimized` arg via autostart's args.
- Windows gotchas: WebView2 runtime (bootstrapper in installer; Win11 ships it); MSVC build tools + Rust `x86_64-pc-windows-msvc`; NSIS or WiX/MSI bundlers. Unsigned installers trigger SmartScreen; options: Azure Trusted Signing, an OV/EV cert, or SignPath Foundation (free for OSS). The updater requires its own minisign keypair (separate from code signing) and a `latest.json` on GitHub Releases. Hidden webview may be throttled; keep logic in Rust.

## 2. Local storage

| Option                                                                   | Pros                                                                                        | Cons                                                                       |
| ------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| `tauri-plugin-sql` 2.6.0 (sqlx under the hood)                           | Frontend can query directly; built-in migrations                                            | Logic in JS, SQL strings in the UI, hard to use from the Rust tracker loop |
| `rusqlite` ✔0.41.1 (+ `rusqlite_migration` ✔2.6.0 or `refinery` ✔0.10.0) | Sync, small, bundled SQLite, simple, ideal for a small local DB; migrations as embedded SQL | Blocking calls; wrap in `spawn_blocking` or a dedicated thread/Mutex       |
| `sqlx` ✔0.9.0                                                            | Async, compile-time checked queries, migrate!                                               | Heavier, compile times, macro setup for little benefit                     |

- **Recommend rusqlite (feature `bundled`) + rusqlite_migration**, DB owned by Rust, exposed to the UI via typed Tauri commands. Enable WAL. Also read GOG's db with rusqlite.
- Use `tauri-plugin-store` only for small settings (steam key, poll interval, theme); keep the Steam API key out of the SQLite DB.

## 3. Process detection (Windows)

- ✔ `sysinfo` 0.40.1: easy, cross-platform; `refresh_processes_specifics` with `ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet)` keeps cost low. Exe path can be missing for protected/elevated processes or anti-cheat protected games.
- ✔ `windows` 0.62.2 / `windows-sys` 0.61.2: `CreateToolhelp32Snapshot`, `QueryFullProcessImageNameW` (needs only `PROCESS_QUERY_LIMITED_INFORMATION`, works for more processes than sysinfo), `GetForegroundWindow` + `GetWindowThreadProcessId`. `windows` = ergonomic, `windows-sys` = raw, smaller compile.
- Recommend: sysinfo for the enumeration, `windows` crate for foreground window/idle. Match running exe paths against a known-install-path index (Steam/Epic/GOG install dirs + user-added exes) rather than names. Avoid heuristics like "any fullscreen app" in v1; offer a manual add list and an optional foreground-required toggle.
- Poll every **5s** for process list (negligible CPU when only exe paths are refreshed); sample foreground/idle every 1-5s only while a game is running. Treat 2 missed polls as session end (debounce launcher restarts).
- Gotcha: many games use a launcher exe that exits and spawns the real one; track by install-directory prefix, not only the launch exe.

## 4. Idle / AFK

- `GetLastInputInfo` (+ `GetTickCount`, handle 32-bit tick wrap; use `GetTickCount64`-style math carefully) via `windows` crate. ✔ `user-idle` 0.6.0 wraps it but was last updated 2023-10; writing the ~15 lines yourself is better.
- **Gotcha:** gamepad input does NOT reset the idle timer (XInput/DirectInput/HID bypass it), so a controller player looks AFK. Mitigation: while a game is in the foreground, poll XInput (`XInputGetState` for slots 0-3, via `windows::Win32::UI::Input::XboxController`, or `gilrs` crate) and treat packet-number changes/stick deviation as activity. Only apply AFK pausing when a game is running; default threshold ~5-10 min, configurable; per-game "disable AFK detection" option. DualShock/DualSense/Switch pads under Steam Input are not visible via XInput: expose the setting and document it.
- Also treat lock screen / session switch as idle (`WTSRegisterSessionNotification` or `WM_POWERBROADCAST` for sleep).

## 5. Steam

- Install path: registry `HKCU\Software\Valve\Steam\SteamPath` (also `HKLM\SOFTWARE\WOW6432Node\Valve\Steam\InstallPath`). ✔ `steamlocate` 2.1.1 does path + library folders + app manifests in one go. ✔ `winreg` 0.56.0 for raw registry.
- Libraries: `<steam>\steamapps\libraryfolders.vdf`; manifests `steamapps\appmanifest_<appid>.acf` (fields `appid`, `name`, `installdir`, `StateFlags`, `LastPlayed`). VDF parsers: ✔ `keyvalues-parser` 0.2.4 / `keyvalues-serde` 0.2.4 (serde support); `steamlocate` already uses them.
- Local playtime: `<steam>\userdata\<accountid>\config\localconfig.vdf` → `UserLocalConfigStore/Software/Valve/Steam/apps/<appid>` with `LastPlayed` (unix) and historically `Playtime`; newer clients do not reliably keep it. Treat as best-effort only. Active account: `HKCU\Software\Valve\Steam\ActiveProcess\ActiveUser` (accountid; SteamID64 = 76561197960265728 + accountid); `config\loginusers.vdf` lists accounts (`MostRecent`). `HKCU\Software\Valve\Steam\RunningAppID` shows the running app: a free signal for Steam games.
- Web API: `GET https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/?key=KEY&steamid=ID64&include_appinfo=1&include_played_free_games=1&format=json` → `games[].appid, name, playtime_forever` (minutes), `rtime_last_played`, `img_icon_url`. Needs a user API key and the profile's "Game details" set to **Public**, else empty. Treat the API as an import/reconcile source, not a live feed.
- Images (verify; Steam is migrating CDN paths): `https://cdn.cloudflare.steamstatic.com/steam/apps/<appid>/header.jpg`, `library_600x900.jpg`, `library_hero.jpg`, `capsule_616x353.jpg`; icons at `https://media.steampowered.com/steamcommunity/public/images/apps/<appid>/<img_icon_url>.jpg`. Local cache: `<steam>\appcache\librarycache\`. Cache downloads on disk.

## 6. Epic Games

- Manifests: `C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests\*.item` (JSON; folder path is stable, but use the registry key `HKLM\SOFTWARE\WOW6432Node\Epic Games\EpicGamesLauncher` `AppDataPath` as fallback).
- Fields: `DisplayName`, `InstallLocation`, `LaunchExecutable` (relative to install dir), `AppName`, `CatalogItemId`, `CatalogNamespace`, `MainGameAppName`, `bIsIncompleteInstall`. Skip entries with `bIsIncompleteInstall` and DLC (where `AppName` ≠ `MainGameAppName`).
- No local playtime and no public playtime API: Epic imports give the library only; tracking starts at install of the tracker. Alternative index: `C:\ProgramData\Epic\UnrealEngineLauncher\LauncherInstalled.dat`.

## 7. GOG Galaxy

- DB: `C:\ProgramData\GOG.com\Galaxy\storage\galaxy-2.0.db` (SQLite).
- Tables (verify with `.schema`; community knowledge from Playnite/GOG plugins): `InstalledBaseProducts`/`InstalledProducts` (installed), `GamePieces` + `GamePieceTypes` (JSON values keyed by `releaseKey` for `title`/`originalTitle`/`meta`), `GameTimes` (`releaseKey`, `minutesInGame`, `userId`), `PlayTasks`/`PlayTaskLaunchParameters` (exe paths). Also the registry `HKLM\SOFTWARE\WOW6432Node\GOG.com\Games\<productId>` (`path`, `exe`, `gameName`) works without Galaxy's DB.
- Gotchas: Galaxy keeps the DB open (WAL). Open **read-only** (`SQLITE_OPEN_READ_ONLY`, optionally `immutable=1` URI) or copy db + `-wal`/`-shm` to a temp dir first; never write. Schema is undocumented and can change. `GameTimes` only reflects play launched through Galaxy. Galaxy is optional: also support registry-only GOG detection.

## 8. Frontend

- Template: `npm create tauri-app@latest` (React + TS, Vite) is current. ✔ Vite 8.3.5, React 19.3.1, TypeScript 7.0.2 (native compiler: check tooling compat, TS 5.x/6 is the safe fallback), Tailwind 4.3.3.
- Charts: **Recharts** ✔3.10.1: simplest React API, SVG, themeable, fits shadcn's chart component (which wraps it). Chart.js ✔4.6.1 (canvas, imperative wrapper), Nivo ✔0.99.0 (pretty, heavier), Tremor ✔`@tremor/react` 3.18.7 (Recharts-based, maintenance has been slow; avoid).
- Data: **TanStack Query** ✔5.105.1 wrapping `invoke()` calls; invalidate on Tauri events (`listen('session-updated')`) emitted from Rust. State: plain React + a small store (Zustand) for UI only.
- UI kit: **shadcn/ui** ✔CLI 4.23.0 (Radix/Base UI + Tailwind 4), dark theme by default, includes charts, tables, dialogs, sidebar. Use `tauri-specta` or `ts-rs` to generate TS types from Rust commands.

## 9. Comparable projects

- **Playnite** https://github.com/JosefNemec/Playnite (C#/WPF): library import from all stores, plugin model, playtime tracking. Borrow: per-library importer abstraction, process-tree watching, GOG/Epic/Steam detection logic (read its `GogLibrary`, `EpicLibrary`, `SteamLibrary` plugins).
- **ActivityWatch** https://github.com/ActivityWatch/activitywatch (Python/Rust/Vue): watcher architecture, AFK watcher (idle + heartbeat model that merges events), local-first data. Borrow: heartbeat merging for sessions.
- **Steam-Playtime / Steam Hours style trackers, GameTime, Gametrack**: small hobby repos; search GitHub topic `playtime-tracker` for current Tauri examples (none seen as dominant; verify).
- **Lutris / Heroic** https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher (Electron+TS): Epic/GOG handling and playtime tracking per game; borrow Epic/GOG metadata and art sourcing.
- **Gaming-Time-Tracker-like Tauri apps**: unverified, do a GitHub search before building.

## 10. Risks / open questions

- Anti-cheat protected games may block exe-path reads; need a name fallback and Steam `RunningAppID`.
- Controller-only AFK false positives (see §4); non-XInput pads.
- Launcher-spawned/child processes and games sharing exes (Java, emulators, Wine-like): need a robust game identification model.
- GOG db schema is undocumented/changing; Epic has no playtime; Steam local playtime unreliable, API needs a key and public profile.
- Unsigned installers/SmartScreen and updater key management; OSS code-signing options.
- Steam CDN image paths are shifting; art licensing/attribution.
- TS 7 / Vite 8 / Tailwind 4 / React 19 ecosystem compatibility with shadcn at install time.
- Data model: sessions vs aggregated totals, reconcile imported vs tracked playtime, timezone/DST handling, crash recovery (write session heartbeat rows so a crash does not lose time).
- Privacy: everything local; Steam API key stored on disk (consider Windows Credential Manager via `keyring`).
