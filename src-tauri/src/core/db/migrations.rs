//! Schema migrations, tracked through SQLite's `user_version`.
//! All timestamps are UTC unix seconds stored as INTEGER.

use std::sync::OnceLock;

use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};

use crate::core::importers::steam::KNOWN_TOOLS;

const V1: &str = "
CREATE TABLE games (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  sort_name TEXT NOT NULL,
  source TEXT NOT NULL DEFAULT 'manual',   -- manual|steam|epic|gog
  source_id TEXT,
  install_dir TEXT,
  icon_path TEXT,
  cover_path TEXT,
  hidden INTEGER NOT NULL DEFAULT 0,
  afk_detection_enabled INTEGER NOT NULL DEFAULT 1,
  created_at INTEGER NOT NULL,
  UNIQUE(source, source_id)
);
CREATE TABLE game_executables (
  id INTEGER PRIMARY KEY,
  game_id INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
  exe_path TEXT NOT NULL,                  -- normalised lowercase full path
  match_mode TEXT NOT NULL DEFAULT 'exact_path', -- exact_path|install_dir_prefix|exe_name
  UNIQUE(game_id, exe_path)
);
CREATE INDEX idx_game_executables_path ON game_executables(exe_path);
CREATE TABLE sessions (
  id INTEGER PRIMARY KEY,
  game_id INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
  started_at INTEGER NOT NULL,
  ended_at INTEGER,
  duration_secs INTEGER NOT NULL DEFAULT 0,
  afk_secs INTEGER NOT NULL DEFAULT 0,
  source TEXT NOT NULL DEFAULT 'auto',     -- auto|manual|imported
  note TEXT
);
CREATE INDEX idx_sessions_game_started ON sessions(game_id, started_at);
CREATE INDEX idx_sessions_started ON sessions(started_at);
CREATE TABLE session_heartbeats (
  session_id INTEGER PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
  last_seen_at INTEGER NOT NULL
);
CREATE TABLE imported_playtime (
  game_id INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
  source TEXT NOT NULL,
  minutes INTEGER NOT NULL,
  fetched_at INTEGER NOT NULL,
  PRIMARY KEY(game_id, source)
);
CREATE TABLE tags (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE
);
CREATE TABLE game_tags (
  game_id INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
  tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
  PRIMARY KEY(game_id, tag_id)
);
CREATE TABLE limits (
  id INTEGER PRIMARY KEY,
  scope TEXT NOT NULL,                     -- global|game
  game_id INTEGER REFERENCES games(id) ON DELETE CASCADE,
  daily_secs INTEGER NOT NULL,
  action TEXT NOT NULL DEFAULT 'notify'    -- notify|notify_repeat
);
";

const V2: &str = "
CREATE TABLE import_sources (
  source TEXT PRIMARY KEY,                 -- steam|epic|gog
  last_scan_at INTEGER NOT NULL
);
";

const V3: &str = "
CREATE TABLE scan_roots (
  id INTEGER PRIMARY KEY,
  path TEXT NOT NULL UNIQUE,
  created_at INTEGER NOT NULL
);
";

const V5: &str = "
CREATE TABLE ignored_imports (
  source TEXT NOT NULL,
  source_id TEXT NOT NULL,
  name TEXT NOT NULL,
  ignored_at INTEGER NOT NULL,
  PRIMARY KEY (source, source_id)
);
";

/// Marks Steam rows imported before the "known tools" rule. Generated from `KNOWN_TOOLS`, so
/// the list lives in one place; the SQL is frozen at what that list was for this release.
fn v4() -> String {
    let ids: Vec<String> = KNOWN_TOOLS
        .iter()
        .map(|(id, _)| format!("'{id}'"))
        .collect();
    format!(
        "UPDATE games SET hidden = 1 WHERE source = 'steam' AND source_id IN ({});",
        ids.join(", ")
    )
}

/// The ordered list of migrations. Append only; never edit a released migration.
fn scripts() -> &'static [String] {
    static SCRIPTS: OnceLock<Vec<String>> = OnceLock::new();
    SCRIPTS.get_or_init(|| vec![V1.into(), V2.into(), V3.into(), v4(), V5.into()])
}

pub fn migrations() -> Migrations<'static> {
    Migrations::new(scripts().iter().map(|sql| M::up(sql)).collect())
}

/// Number of migrations, i.e. the expected `user_version` of an up-to-date database.
pub fn count() -> usize {
    scripts().len()
}

pub fn run(conn: &mut Connection) -> Result<(), rusqlite_migration::Error> {
    migrations().to_latest(conn)
}
