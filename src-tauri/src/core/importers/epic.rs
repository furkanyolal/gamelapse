//! Epic Games: `.item` JSON manifests in the launcher's data folder.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::registry::{Hive, Registry};
use super::{Importer, Scan};
use crate::core::db::repo::normalize_path;
use crate::core::types::{DiscoveredGame, ImportSource, ImportSourceStatus};
use crate::error::{AppError, AppResult};

pub const DEFAULT_MANIFEST_DIR: &str = r"C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests";
const EPIC_KEY: &str = r"SOFTWARE\WOW6432Node\Epic Games\EpicGamesLauncher";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct EpicManifest {
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub install_location: String,
    #[serde(default)]
    pub launch_executable: String,
    #[serde(default)]
    pub app_name: String,
    #[serde(default)]
    pub main_game_app_name: String,
    #[serde(default, rename = "bIsIncompleteInstall")]
    pub is_incomplete_install: bool,
}

pub fn parse_manifest(text: &str) -> AppResult<EpicManifest> {
    serde_json::from_str(text).map_err(|e| AppError::Other(format!("invalid manifest: {e}")))
}

impl EpicManifest {
    /// DLC and add-ons carry the base game's name in `MainGameAppName`.
    fn is_dlc(&self) -> bool {
        !self.main_game_app_name.is_empty() && self.main_game_app_name != self.app_name
    }
}

pub struct EpicImporter {
    registry: Box<dyn Registry>,
    default_dir: PathBuf,
}

impl EpicImporter {
    pub fn new(registry: Box<dyn Registry>) -> Self {
        Self::with_manifest_dir(registry, PathBuf::from(DEFAULT_MANIFEST_DIR))
    }

    pub fn with_manifest_dir(registry: Box<dyn Registry>, default_dir: PathBuf) -> Self {
        Self {
            registry,
            default_dir,
        }
    }

    fn manifest_dir(&self) -> Option<PathBuf> {
        if self.default_dir.is_dir() {
            return Some(self.default_dir.clone());
        }
        let data = self
            .registry
            .get_string(Hive::LocalMachine, EPIC_KEY, "AppDataPath")?;
        Some(Path::new(&data).join("Manifests")).filter(|p| p.is_dir())
    }
}

impl Importer for EpicImporter {
    fn source(&self) -> ImportSource {
        ImportSource::Epic
    }

    fn status(&self) -> ImportSourceStatus {
        let dir = self.manifest_dir();
        ImportSourceStatus {
            source: ImportSource::Epic,
            available: dir.is_some(),
            detail: Some(match dir {
                Some(d) => d.display().to_string(),
                None => "Epic Games Launcher not found".into(),
            }),
            last_scan_at: None,
        }
    }

    fn scan(&self) -> AppResult<Scan> {
        let dir = self
            .manifest_dir()
            .ok_or_else(|| AppError::Other("Epic Games Launcher not found".into()))?;
        let mut scan = Scan::default();
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("item"))
            })
            .collect();
        files.sort();
        for path in files {
            let parsed = std::fs::read_to_string(&path)
                .map_err(AppError::from)
                .and_then(|t| parse_manifest(t.trim_start_matches('\u{feff}')));
            let m = match parsed {
                Ok(m) => m,
                Err(e) => {
                    scan.errors.push(format!("{}: {e}", path.display()));
                    continue;
                }
            };
            if m.is_incomplete_install
                || m.is_dlc()
                || m.app_name.is_empty()
                || m.display_name.is_empty()
                || m.install_location.is_empty()
            {
                continue;
            }
            let install = Path::new(&m.install_location);
            if !install.is_dir() {
                continue;
            }
            let executables = if m.launch_executable.is_empty() {
                Vec::new()
            } else {
                vec![normalize_path(
                    &install.join(&m.launch_executable).to_string_lossy(),
                )]
            };
            scan.games.push(DiscoveredGame {
                source: ImportSource::Epic,
                source_id: m.app_name,
                name: m.display_name,
                install_dir: Some(m.install_location.clone()),
                executables,
                playtime_minutes: None,
                already_imported: false,
            });
        }
        Ok(scan)
    }
}
