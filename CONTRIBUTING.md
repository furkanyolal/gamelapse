# Contributing

Thanks for helping. Bug reports, launcher fixes and pull requests are all welcome.

## Setup

1. Install Rust (stable, `x86_64-pc-windows-msvc`) with the Visual Studio C++ build tools, and Node.js 24.
2. `npm install`
3. `npm run tauri dev` to run the app with hot reload.

## Where things are

- [`AGENTS.md`](AGENTS.md) describes the repository layout and the rules for working in it.
- [`docs/contract.md`](docs/contract.md) is the IPC spec: every command, event, payload type and tracker rule. A change to the contract comes first, code follows.

## Scripts

| Command                                                      | Purpose                           |
| ------------------------------------------------------------ | --------------------------------- |
| `npm run dev`                                                | Vite dev server only              |
| `npm run build`                                              | Type-check and build the frontend |
| `npm run lint`                                               | ESLint                            |
| `npm run format` / `npm run format:check`                    | Prettier                          |
| `npm test`                                                   | Vitest                            |
| `npm run tauri dev`                                          | Full app                          |
| `cargo fmt/clippy/test --manifest-path src-tauri/Cargo.toml` | Rust checks                       |

`cargo test` also regenerates `src/bindings.ts`. Commit it when it changes. Never edit it by hand.

## Commits

Use [Conventional Commits](https://www.conventionalcommits.org): `feat:`, `fix:`, `docs:`, `chore:`, `refactor:`, `test:`.

## Adding a command

1. Write `#[tauri::command] #[specta::specta] pub fn ...` in `src-tauri/src/commands/`. Argument and return types derive `specta::Type`. Errors use `AppResult`.
2. Add it to `collect_commands![...]` in `specta_builder()` in `src-tauri/src/lib.rs`.
3. Run `cargo test --manifest-path src-tauri/Cargo.toml` to regenerate `src/bindings.ts`, and commit it.
4. Add a hook in `src/hooks/` that calls `commands.<name>()` with TanStack Query.
5. Document the command in `docs/contract.md`.

## Adding a migration

Append a new migration in `src-tauri/src/core/db/migrations.rs`. Never edit a released one. The schema version is tracked by SQLite's `user_version`. Timestamps are UTC unix seconds.

## Running a single Rust test

```sh
cargo test --manifest-path src-tauri/Cargo.toml <test_name>
```

Add `-- --nocapture` to see output. Use `--test <file>` to run one integration test file from `src-tauri/tests/`.

## Before opening a PR

```sh
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
npm run lint
npm run format:check
npm test -- --run
npm run build
```

## Reporting a launcher import problem

Import is tested with fixture files, so real-world reports are valuable. Open an issue and say which launcher is involved. Attach the following, with personal paths removed if you like:

- **Steam:** one `appmanifest_*.acf` from the game's library, in `steamapps` (for example `D:\SteamLibrary\steamapps`).
- **Epic Games:** the game's `.item` file from `C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests`.
- **GOG Galaxy:** the values (`gameName`, `path`, `exe`) of the game's key under `HKEY_LOCAL_MACHINE\SOFTWARE\WOW6432Node\GOG.com\Games\<id>`. Export the key from `regedit`, or copy the values.
- **Heroic:** the game's entry from `%APPDATA%\heroic\legendaryConfig\legendary\installed.json` (Epic) or `%APPDATA%\heroic\gog_store\installed.json` (GOG).

Also attach the relevant part of the log from `%LOCALAPPDATA%\com.furkanyolal.gamelapse\logs`.
