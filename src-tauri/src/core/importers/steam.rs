//! Steam: `libraryfolders.vdf` for the libraries, `appmanifest_*.acf` for the games.

use std::path::{Path, PathBuf};

use keyvalues_parser::{Value, Vdf};

use super::registry::{Hive, Registry, STEAM_KEY};
use super::{Importer, Scan};
use crate::core::types::{DiscoveredGame, ImportSource, ImportSourceStatus};
use crate::error::{AppError, AppResult};

/// Steamworks Common Redistributables; installed next to games but not a game.
const REDISTRIBUTABLES_APPID: &str = "228980";
const HKLM_STEAM_KEY: &str = r"SOFTWARE\WOW6432Node\Valve\Steam";

pub struct SteamImporter {
    registry: Box<dyn Registry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub app_id: String,
    pub name: String,
    pub install_dir: String,
    pub state_flags: Option<u32>,
}

fn field<'a>(obj: &'a keyvalues_parser::Obj<'_>, key: &str) -> Option<&'a str> {
    // Steam writes keys in mixed case (`appid`, `StateFlags`); match case-insensitively.
    obj.iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .and_then(|(_, values)| values.first())
        .and_then(Value::get_str)
}

fn parse_error(what: &str, e: impl std::fmt::Display) -> AppError {
    AppError::Other(format!("{what}: {e}"))
}

/// Library root paths listed in `libraryfolders.vdf`, in file order. Understands both the
/// current format (numbered objects with a `path`) and the old one (numbered string values).
pub fn parse_library_folders(text: &str) -> AppResult<Vec<PathBuf>> {
    let vdf = keyvalues_parser::parse(text)
        .map(Vdf::from)
        .map_err(|e| parse_error("libraryfolders.vdf", e))?;
    let Some(root) = vdf.value.get_obj() else {
        return Err(AppError::Other("libraryfolders.vdf: no libraries".into()));
    };
    let mut libraries = Vec::new();
    for (key, values) in root.iter() {
        if key.is_empty() || !key.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let Some(value) = values.first() else {
            continue;
        };
        let path = match value {
            Value::Obj(obj) => field(obj, "path"),
            Value::Str(s) => Some(s.as_ref()),
        };
        if let Some(path) = path.filter(|p| !p.is_empty()) {
            libraries.push(PathBuf::from(path));
        }
    }
    Ok(libraries)
}

pub fn parse_manifest(text: &str) -> AppResult<Manifest> {
    let vdf = keyvalues_parser::parse(text)
        .map(Vdf::from)
        .map_err(|e| parse_error("appmanifest", e))?;
    let obj = vdf
        .value
        .get_obj()
        .ok_or_else(|| AppError::Other("appmanifest: not an object".into()))?;
    let required = |key: &str| {
        field(obj, key)
            .map(str::to_string)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| AppError::Other(format!("appmanifest: missing {key}")))
    };
    Ok(Manifest {
        app_id: required("appid")?,
        name: required("name")?,
        install_dir: required("installdir")?,
        state_flags: field(obj, "StateFlags").and_then(|v| v.parse().ok()),
    })
}

fn has_libraryfolders(root: &Path) -> bool {
    root.join("steamapps").join("libraryfolders.vdf").is_file()
}

/// Steam's install path: `HKCU\...\SteamPath` when it really holds `libraryfolders.vdf`
/// (it can go stale after a reinstall), otherwise the machine-wide `InstallPath`.
pub fn resolve_steam_path(registry: &dyn Registry) -> Option<PathBuf> {
    let user = registry
        .get_string(Hive::CurrentUser, STEAM_KEY, "SteamPath")
        .map(|p| PathBuf::from(p.replace('/', "\\")))
        .filter(|p| has_libraryfolders(p));
    user.or_else(|| {
        registry
            .get_string(Hive::LocalMachine, HKLM_STEAM_KEY, "InstallPath")
            .map(|p| PathBuf::from(p.replace('/', "\\")))
            .filter(|p| p.is_dir())
    })
}

impl SteamImporter {
    pub fn new(registry: Box<dyn Registry>) -> Self {
        Self { registry }
    }

    /// Library roots to scan: the ones in `libraryfolders.vdf`, plus Steam's own folder.
    fn libraries(&self, steam: &Path, errors: &mut Vec<String>) -> Vec<PathBuf> {
        let mut libraries = Vec::new();
        let vdf = steam.join("steamapps").join("libraryfolders.vdf");
        match std::fs::read_to_string(&vdf)
            .map_err(AppError::from)
            .and_then(|text| parse_library_folders(&text))
        {
            Ok(found) => libraries = found,
            Err(e) => errors.push(format!("{}: {e}", vdf.display())),
        }
        libraries.insert(0, steam.to_path_buf());
        let mut seen = std::collections::HashSet::new();
        libraries.retain(|p| seen.insert(p.to_string_lossy().to_lowercase()));
        libraries
    }
}

impl Importer for SteamImporter {
    fn source(&self) -> ImportSource {
        ImportSource::Steam
    }

    fn status(&self) -> ImportSourceStatus {
        let path = resolve_steam_path(self.registry.as_ref());
        ImportSourceStatus {
            source: ImportSource::Steam,
            available: path.is_some(),
            detail: Some(match path {
                Some(p) => p.display().to_string(),
                None => "Steam install not found".into(),
            }),
            last_scan_at: None,
        }
    }

    fn scan(&self) -> AppResult<Scan> {
        let steam = resolve_steam_path(self.registry.as_ref())
            .ok_or_else(|| AppError::Other("Steam install not found".into()))?;
        let mut scan = Scan::default();
        let libraries = self.libraries(&steam, &mut scan.errors);
        for library in libraries {
            let steamapps = library.join("steamapps");
            let Ok(entries) = std::fs::read_dir(&steamapps) else {
                // A library on a disconnected drive is normal.
                continue;
            };
            let mut manifests: Vec<PathBuf> = entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("appmanifest_") && n.ends_with(".acf"))
                })
                .collect();
            manifests.sort();
            for path in manifests {
                let parsed = std::fs::read_to_string(&path)
                    .map_err(AppError::from)
                    .and_then(|text| parse_manifest(&text));
                let manifest = match parsed {
                    Ok(m) => m,
                    Err(e) => {
                        scan.errors.push(format!("{}: {e}", path.display()));
                        continue;
                    }
                };
                if manifest.app_id == REDISTRIBUTABLES_APPID {
                    continue;
                }
                let install_dir = steamapps.join("common").join(&manifest.install_dir);
                if !install_dir.is_dir() {
                    continue;
                }
                scan.games.push(DiscoveredGame {
                    source: ImportSource::Steam,
                    source_id: manifest.app_id,
                    name: manifest.name,
                    install_dir: Some(install_dir.display().to_string()),
                    executables: Vec::new(),
                    playtime_minutes: None,
                    already_imported: false,
                    game_source: None,
                });
            }
        }
        // The same app can appear in two libraries (stale manifest); keep the first.
        let mut seen = std::collections::HashSet::new();
        scan.games.retain(|g| seen.insert(g.source_id.clone()));
        Ok(scan)
    }
}
