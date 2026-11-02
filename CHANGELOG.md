# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-11-02

First public release.

### Added

- Tracking
  - Automatic detection of running games by polling processes, matched by executable path or install folder
  - Manual timer
  - Option to count a game only while its window is in the foreground
  - Session heartbeats and crash recovery
  - Merging of sessions separated by a short gap
- AFK detection
  - Keyboard, mouse, controller and lock screen
  - Configurable threshold, 10 minutes by default
  - AFK time is excluded from session duration and limits
- Dashboard and stats
  - Today, this week and all-time totals
  - Charts: daily, weekly trend, top games and time of day
  - Session history with editing
- Limits and reminders
  - Daily limits per game or for all games
  - Play reminders at a chosen interval
  - Windows notifications
- Library
  - Import from Steam, Epic Games and GOG Galaxy, with cover art
  - Import from Heroic Games Launcher (Epic, GOG and Amazon games), with playtime and covers
  - Games folders: confirm-first scan of folders you add, with suggested names and executables; nothing is scanned or added without your click
  - Name and cover lookup through Steam's store search, per scan candidate and from a "Find cover" button
  - Import on startup
  - Launcher-reported hours count once toward a game's all-time total and are never double counted
  - Hidden games stay tracked but are left out of lists, totals and the now-playing strip; Steam tools such as Wallpaper Engine and Lossless Scaling are hidden automatically
  - Deleting an imported game keeps it from coming back on the next import, until you restore it from the Import dialog
  - Optional Steam Web API import of past playtime
  - Tags, merging of duplicate games and custom covers
- Data
  - CSV and JSON export of sessions
  - Backup and restore of the database
  - Legacy data migration from the pre-release app name
- System
  - System tray
  - Start with Windows
  - Auto-update from GitHub Releases
  - Dark, light and system themes

[1.0.0]: https://github.com/furkanyolal/gamelapse/releases/tag/v1.0.0
