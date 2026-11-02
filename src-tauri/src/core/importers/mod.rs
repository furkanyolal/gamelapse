//! Library importers (Steam, Epic, GOG). Registry and filesystem access are injectable so
//! the parsers run against fixtures; persistence lives in [`store`].

pub mod covers;
pub mod epic;
pub mod gog;
pub mod heroic;
pub mod http;
pub mod registry;
pub mod steam;
pub mod steam_api;
pub mod store;

use rusqlite::{Connection, params};

use crate::core::types::{DiscoveredGame, ImportSource, ImportSourceStatus};
use crate::error::AppResult;

/// A cover image to download for a discovered game, by launcher-supplied URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverSource {
    pub source_id: String,
    /// Folder under `<app data>/covers`, e.g. `epic`.
    pub subdir: &'static str,
    pub url: String,
}

/// What one scan found. `errors` are non-fatal problems (a bad manifest, a Galaxy schema
/// surprise); the games that did parse are still returned.
#[derive(Debug, Default)]
pub struct Scan {
    pub games: Vec<DiscoveredGame>,
    pub errors: Vec<String>,
    /// Cover URLs the launcher knows; empty for importers whose art is fetched elsewhere.
    pub covers: Vec<CoverSource>,
}

/// A source of installed games.
pub trait Importer {
    fn source(&self) -> ImportSource;
    /// Whether the launcher is installed. `last_scan_at` is filled in by the caller.
    fn status(&self) -> ImportSourceStatus;
    /// Reads the launcher's files. Never writes. `already_imported` is left false.
    fn scan(&self) -> AppResult<Scan>;
}

/// The importer for `source`, wired to the real registry and default launcher paths.
pub fn importer_for(source: ImportSource) -> Box<dyn Importer> {
    let registry = || Box::new(registry::WinRegistry) as Box<dyn registry::Registry>;
    match source {
        ImportSource::Steam => Box::new(steam::SteamImporter::new(registry())),
        ImportSource::Epic => Box::new(epic::EpicImporter::new(registry())),
        ImportSource::Gog => Box::new(gog::GogImporter::new(registry())),
        ImportSource::Heroic => Box::new(heroic::HeroicImporter::new()),
    }
}

/// Sets `already_imported` on games that exist in the library and `ignored` on tombstoned ones.
pub fn mark_imported(conn: &Connection, games: &mut [DiscoveredGame]) -> AppResult<()> {
    let mut stmt = conn.prepare("SELECT 1 FROM games WHERE source = ?1 AND source_id = ?2")?;
    let mut ignored =
        conn.prepare("SELECT 1 FROM ignored_imports WHERE source = ?1 AND source_id = ?2")?;
    for game in games {
        game.already_imported =
            stmt.exists(params![game.library_source().as_str(), game.source_id])?;
        game.ignored = ignored.exists(params![game.library_source().as_str(), game.source_id])?;
    }
    Ok(())
}

/// `last_scan_at` per source from `import_sources`.
pub fn last_scan_at(conn: &Connection, source: ImportSource) -> AppResult<Option<i64>> {
    use rusqlite::OptionalExtension;
    Ok(conn
        .query_row(
            "SELECT last_scan_at FROM import_sources WHERE source = ?1",
            [source.as_str()],
            |r| r.get(0),
        )
        .optional()?)
}
