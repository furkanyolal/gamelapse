# Contributing

## Setup

1. Install Rust (stable, `x86_64-pc-windows-msvc`) with the Visual Studio C++ build tools, and Node.js 24.
2. `npm install`
3. `npm run tauri dev` to run the app with hot reload.

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

`cargo test` also regenerates `src/bindings.ts`; commit it when it changes.

## Commits

Use [Conventional Commits](https://www.conventionalcommits.org): `feat:`, `fix:`, `docs:`, `chore:`, `refactor:`, `test:`.

## Before opening a PR

Run lint and tests for both sides and make sure they pass:

```sh
npm run lint && npm run format:check && npm test -- --run && npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```
