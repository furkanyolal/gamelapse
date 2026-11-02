# Gamelapse

Gamelapse is an open source game time tracker for Windows 10 and 11. It lives in the system tray, detects when your games are running, and records how long you play, with a dashboard for browsing your library, sessions and statistics. Everything is stored locally on your machine.

**Status: early development.** Nothing is usable yet; this repository currently contains the project skeleton.

## Stack

- Rust and [Tauri 2](https://v2.tauri.app) (tray, background tracking, SQLite via `rusqlite`)
- React, TypeScript, Vite, Tailwind CSS 4 and shadcn/ui
- TanStack Query, React Router, Vitest

## Running

Requires Rust (MSVC toolchain), Node.js 24 and the WebView2 runtime.

```sh
npm install
npm run tauri dev
```

## License

[MIT](LICENSE)
