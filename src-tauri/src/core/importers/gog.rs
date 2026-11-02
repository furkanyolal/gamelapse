//! GOG: the registry lists installed games; Galaxy's SQLite database adds titles and playtime.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use super::registry::{Hive, Registry};
use super::{Importer, Scan};
use crate::core::db::repo::normalize_path;
use crate::core::types::{DiscoveredGame, ImportSource, ImportSourceStatus};
use crate::error::{AppError, AppResult};

pub const DEFAULT_GALAXY_DB: &str = r"C:\ProgramData\GOG.com\Galaxy\storage\galaxy-2.0.db";
const GAMES_KEY: &str = r"SOFTWARE\WOW6432Node\GOG.com\Games";

pub struct GogImporter {
    registry: Box<dyn Registry>,
    galaxy_db: PathBuf,
}

/// What Galaxy knows per product id.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct GalaxyInfo {
    pub title: Option<String>,
    pub minutes: Option<i64>,
}

impl GogImporter {
    pub fn new(registry: Box<dyn Registry>) -> Self {
        Self::with_galaxy_db(registry, PathBuf::from(DEFAULT_GALAXY_DB))
    }

    pub fn with_galaxy_db(registry: Box<dyn Registry>, galaxy_db: PathBuf) -> Self {
        Self {
            registry,
            galaxy_db,
        }
    }

    fn registry_games(&self) -> Vec<DiscoveredGame> {
        let Some(ids) = self.registry.subkeys(Hive::LocalMachine, GAMES_KEY) else {
            return Vec::new();
        };
        let mut games = Vec::new();
        for id in ids {
            let key = format!(r"{GAMES_KEY}\{id}");
            let get = |name: &str| {
                self.registry
                    .get_string(Hive::LocalMachine, &key, name)
                    .filter(|v| !v.trim().is_empty())
            };
            // A missing name is filled from Galaxy's titles later; nameless games are dropped.
            let name = get("gameName").unwrap_or_default();
            let Some(path) = get("path").filter(|p| Path::new(p).is_dir()) else {
                continue;
            };
            let path = Some(path);
            let executables = match (&path, get("exe")) {
                (Some(dir), Some(exe)) => {
                    let exe_path = Path::new(&exe);
                    let full = if exe_path.is_absolute() {
                        exe_path.to_path_buf()
                    } else {
                        Path::new(dir).join(exe_path)
                    };
                    vec![normalize_path(&full.to_string_lossy())]
                }
                _ => Vec::new(),
            };
            games.push(DiscoveredGame {
                source: ImportSource::Gog,
                source_id: id,
                name,
                install_dir: path,
                executables,
                playtime_minutes: None,
                already_imported: false,
                game_source: None,
            });
        }
        games.sort_by(|a, b| a.source_id.cmp(&b.source_id));
        games
    }
}

/// Copies the database (and its WAL files, which Galaxy keeps open) to a temp dir and reads
/// it read-only. Never touches the original.
pub fn read_galaxy_db(db: &Path) -> AppResult<HashMap<String, GalaxyInfo>> {
    let dir = tempfile::tempdir()?;
    let copy = dir.path().join("galaxy.db");
    std::fs::copy(db, &copy)?;
    for suffix in ["-wal", "-shm"] {
        let mut from = db.as_os_str().to_owned();
        from.push(suffix);
        let mut to = copy.as_os_str().to_owned();
        to.push(suffix);
        if Path::new(&from).is_file() {
            let _ = std::fs::copy(&from, &to);
        }
    }
    let conn = Connection::open_with_flags(&copy, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    query_galaxy(&conn)
}

/// Release keys look like `gog_1207658930`; the product id is the part after the underscore.
fn product_id(release_key: &str) -> Option<&str> {
    release_key.strip_prefix("gog_").filter(|id| !id.is_empty())
}

pub fn query_galaxy(conn: &Connection) -> AppResult<HashMap<String, GalaxyInfo>> {
    let mut info: HashMap<String, GalaxyInfo> = HashMap::new();

    let mut stmt = conn.prepare(
        "SELECT gp.releaseKey, gp.value FROM GamePieces gp
         JOIN GamePieceTypes gpt ON gpt.id = gp.gamePieceTypeId
         WHERE gpt.type = 'title'",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    for row in rows {
        let (key, value) = row?;
        let Some(id) = product_id(&key) else { continue };
        let title = serde_json::from_str::<serde_json::Value>(&value)
            .ok()
            .and_then(|v| v.get("title").and_then(|t| t.as_str()).map(str::to_string))
            .filter(|t| !t.is_empty());
        info.entry(id.to_string()).or_default().title = title;
    }

    let mut stmt =
        conn.prepare("SELECT releaseKey, SUM(minutesInGame) FROM GameTimes GROUP BY releaseKey")?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?))
    })?;
    for row in rows {
        let (key, minutes) = row?;
        let Some(id) = product_id(&key) else { continue };
        info.entry(id.to_string()).or_default().minutes = minutes;
    }
    Ok(info)
}

impl Importer for GogImporter {
    fn source(&self) -> ImportSource {
        ImportSource::Gog
    }

    fn status(&self) -> ImportSourceStatus {
        let has_games_key = self
            .registry
            .subkeys(Hive::LocalMachine, GAMES_KEY)
            .is_some();
        let has_db = self.galaxy_db.is_file();
        ImportSourceStatus {
            source: ImportSource::Gog,
            available: has_games_key || has_db,
            detail: Some(if has_db {
                self.galaxy_db.display().to_string()
            } else if has_games_key {
                "GOG registry entries".into()
            } else {
                "GOG Galaxy not found".into()
            }),
            last_scan_at: None,
        }
    }

    fn scan(&self) -> AppResult<Scan> {
        if !self.status().available {
            return Err(AppError::Other("GOG Galaxy not found".into()));
        }
        let mut scan = Scan {
            games: self.registry_games(),
            ..Scan::default()
        };
        if self.galaxy_db.is_file() {
            match read_galaxy_db(&self.galaxy_db) {
                Ok(info) => {
                    for game in &mut scan.games {
                        let Some(extra) = info.get(&game.source_id) else {
                            continue;
                        };
                        game.playtime_minutes = extra.minutes;
                        if let Some(title) = &extra.title
                            && game.name.trim().is_empty()
                        {
                            game.name = title.clone();
                        }
                    }
                }
                Err(e) => scan
                    .errors
                    .push(format!("GOG Galaxy database could not be read: {e}")),
            }
        }
        scan.games.retain(|g| !g.name.trim().is_empty());
        Ok(scan)
    }
}
