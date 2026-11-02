//! Heroic Games Launcher: reads its config folder (`%APPDATA%\heroic`) read-only. Heroic
//! manages Epic (legendary), GOG (gogdl), Amazon (nile) and sideloaded games; each is stored
//! under the library source it belongs to, so a game seen by both the Epic launcher and
//! Heroic appears once.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{CoverSource, Importer, Scan};
use crate::core::db::repo::normalize_path;
use crate::core::types::{DiscoveredGame, GameSource, ImportSource, ImportSourceStatus};
use crate::error::{AppError, AppResult};

const LEGENDARY_DIR: &str = r"legendaryConfig\legendary";
const GOG_INSTALLED: &str = r"gog_store\installed.json";
const GOG_LIBRARY: &str = r"gog_store\library.json";
const NILE_INSTALLED: &str = r"nile_config\nile\installed.json";
const NILE_LIBRARY: &str = r"nile_config\nile\library.json";
const SIDELOAD_LIBRARY: &str = r"sideload_apps\library.json";
/// Per-game play time kept by Heroic itself: `{ appName: { totalPlayed: minutes } }`.
const PLAYTIME: &str = r"store\timestamp.json";

pub struct HeroicImporter {
    config_dir: PathBuf,
}

impl Default for HeroicImporter {
    fn default() -> Self {
        Self::new()
    }
}

impl HeroicImporter {
    /// Uses `%APPDATA%\heroic`.
    pub fn new() -> Self {
        let dir = std::env::var_os("APPDATA")
            .map(|a| PathBuf::from(a).join("heroic"))
            .unwrap_or_default();
        Self::with_config_dir(dir)
    }

    pub fn with_config_dir(config_dir: PathBuf) -> Self {
        Self { config_dir }
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.config_dir.join(rel)
    }

    fn legendary_installed(&self) -> PathBuf {
        self.path(LEGENDARY_DIR).join("installed.json")
    }

    fn has_data(&self) -> bool {
        self.config_dir.is_dir()
            && [
                self.legendary_installed(),
                self.path(GOG_INSTALLED),
                self.path(NILE_INSTALLED),
                self.path(SIDELOAD_LIBRARY),
            ]
            .iter()
            .any(|p| p.is_file())
    }
}

fn read_json(path: &Path) -> AppResult<Value> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| AppError::Other(format!("{}: {e}", path.display())))?;
    serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|e| AppError::Other(format!("{}: invalid JSON: {e}", path.display())))
}

fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// The first non-empty string among `keys`.
fn first_str<'a>(v: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|k| str_of(v, k))
}

/// Windows entries only; an absent platform counts as Windows.
fn is_windows(v: &Value) -> bool {
    match str_of(v, "platform") {
        None => true,
        Some(p) => matches!(p.to_ascii_lowercase().as_str(), "windows" | "win32" | "win"),
    }
}

/// The entries of a Heroic list file: a bare array, or an object holding one under `keys`.
fn list_entries<'a>(root: &'a Value, keys: &[&str]) -> Vec<&'a Value> {
    if let Some(arr) = root.as_array() {
        return arr.iter().collect();
    }
    for key in keys {
        if let Some(arr) = root.get(*key).and_then(Value::as_array) {
            return arr.iter().collect();
        }
    }
    Vec::new()
}

/// What a store's `library.json` says about a game.
#[derive(Default)]
struct LibraryInfo {
    title: Option<String>,
    art: Option<String>,
}

fn load_library(path: &Path, errors: &mut Vec<String>) -> HashMap<String, LibraryInfo> {
    let mut out = HashMap::new();
    if !path.is_file() {
        return out;
    }
    let root = match read_json(path) {
        Ok(v) => v,
        Err(e) => {
            errors.push(e.to_string());
            return out;
        }
    };
    for entry in list_entries(&root, &["games", "library"]) {
        let Some(id) = first_str(entry, &["app_name", "appName", "id"]) else {
            continue;
        };
        let title = first_str(entry, &["title"])
            .or_else(|| entry.get("product").and_then(|p| str_of(p, "title")))
            .map(str::to_string);
        let art = first_str(entry, &["art_cover", "art_square"]).map(str::to_string);
        out.insert(id.to_string(), LibraryInfo { title, art });
    }
    out
}

/// `{ appName: totalPlayed }` in minutes from Heroic's own tracking.
fn load_playtime(path: &Path, errors: &mut Vec<String>) -> HashMap<String, i64> {
    let mut out = HashMap::new();
    if !path.is_file() {
        return out;
    }
    match read_json(path) {
        Ok(Value::Object(map)) => {
            for (name, entry) in map {
                if let Some(minutes) = entry.get("totalPlayed").and_then(Value::as_f64)
                    && minutes > 0.0
                {
                    out.insert(name, minutes.round() as i64);
                }
            }
        }
        Ok(_) => {}
        Err(e) => errors.push(e.to_string()),
    }
    out
}

/// The URL of a legendary cover: `DieselGameBoxTall`, else `DieselGameBox`.
pub fn epic_cover_url(metadata: &Value) -> Option<String> {
    let images = metadata
        .get("metadata")
        .unwrap_or(metadata)
        .get("keyImages")?
        .as_array()?;
    let pick = |kind: &str| {
        images
            .iter()
            .find(|i| str_of(i, "type") == Some(kind))
            .and_then(|i| str_of(i, "url"))
            .map(str::to_string)
    };
    pick("DieselGameBoxTall").or_else(|| pick("DieselGameBox"))
}

/// GOG serves art as `.webp` in Heroic's library; the same image exists as `.jpg`.
fn gog_cover_url(url: &str) -> String {
    match url.strip_suffix(".webp") {
        Some(base) => format!("{base}.jpg"),
        None => url.to_string(),
    }
}

/// The main executable named by GOG's `goggame-<id>.info` in the install folder.
fn gog_primary_exe(install: &Path, app_name: &str) -> Option<String> {
    let text = std::fs::read_to_string(install.join(format!("goggame-{app_name}.info"))).ok()?;
    let info: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let tasks = info.get("playTasks")?.as_array()?;
    let task = tasks
        .iter()
        .find(|t| t.get("isPrimary").and_then(Value::as_bool) == Some(true))?;
    let rel = str_of(task, "path")?;
    Some(normalize_path(&install.join(rel).to_string_lossy()))
}

struct Found {
    game_source: GameSource,
    source_id: String,
    name: String,
    install_dir: String,
    executables: Vec<String>,
    cover: Option<(&'static str, String)>,
}

impl HeroicImporter {
    fn epic(&self, errors: &mut Vec<String>, out: &mut Vec<Found>) {
        let path = self.legendary_installed();
        if !path.is_file() {
            return;
        }
        let root = match read_json(&path) {
            Ok(v) => v,
            Err(e) => {
                errors.push(e.to_string());
                return;
            }
        };
        let Some(map) = root.as_object() else {
            errors.push(format!("{}: unexpected layout", path.display()));
            return;
        };
        for (key, entry) in map {
            let app_name = str_of(entry, "app_name").unwrap_or(key);
            let is_dlc = entry.get("is_dlc").and_then(Value::as_bool) == Some(true);
            let (Some(title), Some(install)) =
                (str_of(entry, "title"), str_of(entry, "install_path"))
            else {
                continue;
            };
            if is_dlc || !is_windows(entry) || !Path::new(install).is_dir() {
                continue;
            }
            let executables = match str_of(entry, "executable") {
                Some(exe) => vec![normalize_path(
                    &Path::new(install).join(exe).to_string_lossy(),
                )],
                None => Vec::new(),
            };
            let cover = read_json(
                &self
                    .path(LEGENDARY_DIR)
                    .join("metadata")
                    .join(format!("{app_name}.json")),
            )
            .ok()
            .and_then(|m| epic_cover_url(&m))
            .map(|url| ("epic", url));
            out.push(Found {
                game_source: GameSource::Epic,
                source_id: app_name.to_string(),
                name: title.to_string(),
                install_dir: install.to_string(),
                executables,
                cover,
            });
        }
    }

    fn gog(&self, errors: &mut Vec<String>, out: &mut Vec<Found>) {
        let path = self.path(GOG_INSTALLED);
        if !path.is_file() {
            return;
        }
        let root = match read_json(&path) {
            Ok(v) => v,
            Err(e) => {
                errors.push(e.to_string());
                return;
            }
        };
        let library = load_library(&self.path(GOG_LIBRARY), errors);
        for entry in list_entries(&root, &["installed"]) {
            let (Some(app_name), Some(install)) = (
                first_str(entry, &["appName", "app_name"]),
                first_str(entry, &["install_path"]),
            ) else {
                continue;
            };
            if str_of(entry, "platform").is_some_and(|p| !p.eq_ignore_ascii_case("windows"))
                || !Path::new(install).is_dir()
            {
                continue;
            }
            let info = library.get(app_name);
            let name = info
                .and_then(|i| i.title.clone())
                .or_else(|| {
                    Path::new(install)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| app_name.to_string());
            out.push(Found {
                game_source: GameSource::Gog,
                source_id: app_name.to_string(),
                name,
                install_dir: install.to_string(),
                executables: gog_primary_exe(Path::new(install), app_name)
                    .into_iter()
                    .collect(),
                cover: info
                    .and_then(|i| i.art.as_deref())
                    .map(|a| ("gog", gog_cover_url(a))),
            });
        }
    }

    fn amazon(&self, errors: &mut Vec<String>, out: &mut Vec<Found>) {
        let path = self.path(NILE_INSTALLED);
        if !path.is_file() {
            return;
        }
        let root = match read_json(&path) {
            Ok(v) => v,
            Err(e) => {
                errors.push(e.to_string());
                return;
            }
        };
        let library = load_library(&self.path(NILE_LIBRARY), errors);
        for entry in list_entries(&root, &["installed"]) {
            let (Some(id), Some(install)) = (
                first_str(entry, &["id", "appName", "app_name"]),
                first_str(entry, &["path", "install_path"]),
            ) else {
                continue;
            };
            if !Path::new(install).is_dir() {
                continue;
            }
            let info = library.get(id);
            let name = info
                .and_then(|i| i.title.clone())
                .or_else(|| {
                    Path::new(install)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| id.to_string());
            out.push(Found {
                game_source: GameSource::Amazon,
                source_id: id.to_string(),
                name,
                install_dir: install.to_string(),
                executables: Vec::new(),
                cover: info
                    .and_then(|i| i.art.as_deref())
                    .map(|a| ("amazon", a.to_string())),
            });
        }
    }

    fn sideload(&self, errors: &mut Vec<String>, out: &mut Vec<Found>) {
        let path = self.path(SIDELOAD_LIBRARY);
        if !path.is_file() {
            return;
        }
        let root = match read_json(&path) {
            Ok(v) => v,
            Err(e) => {
                errors.push(e.to_string());
                return;
            }
        };
        for entry in list_entries(&root, &["games", "library"]) {
            let (Some(app_name), Some(title)) = (
                first_str(entry, &["app_name", "appName"]),
                first_str(entry, &["title"]),
            ) else {
                continue;
            };
            if entry.get("is_installed").and_then(Value::as_bool) == Some(false) {
                continue;
            }
            let install = entry.get("install").unwrap_or(&Value::Null);
            if !is_windows(install) {
                continue;
            }
            let exe = str_of(install, "executable").map(PathBuf::from);
            let dir = str_of(entry, "folder_name")
                .map(PathBuf::from)
                .filter(|d| d.is_dir())
                .or_else(|| {
                    exe.as_ref()
                        .and_then(|e| e.parent())
                        .map(Path::to_path_buf)
                        .filter(|d| d.is_dir())
                });
            let Some(dir) = dir else { continue };
            out.push(Found {
                game_source: GameSource::Manual,
                source_id: format!("heroic-sideload:{app_name}"),
                name: title.to_string(),
                install_dir: dir.display().to_string(),
                executables: exe
                    .filter(|e| e.is_file())
                    .map(|e| normalize_path(&e.to_string_lossy()))
                    .into_iter()
                    .collect(),
                cover: first_str(entry, &["art_cover", "art_square"])
                    .filter(|u| u.starts_with("http"))
                    .map(|u| ("heroic", u.to_string())),
            });
        }
    }
}

impl Importer for HeroicImporter {
    fn source(&self) -> ImportSource {
        ImportSource::Heroic
    }

    fn status(&self) -> ImportSourceStatus {
        let available = self.has_data();
        ImportSourceStatus {
            source: ImportSource::Heroic,
            available,
            detail: Some(if available {
                self.config_dir.display().to_string()
            } else {
                "Heroic Games Launcher not found".into()
            }),
            last_scan_at: None,
        }
    }

    fn scan(&self) -> AppResult<Scan> {
        if !self.has_data() {
            return Err(AppError::Other("Heroic Games Launcher not found".into()));
        }
        let mut scan = Scan::default();
        let mut found = Vec::new();
        self.epic(&mut scan.errors, &mut found);
        self.gog(&mut scan.errors, &mut found);
        self.amazon(&mut scan.errors, &mut found);
        self.sideload(&mut scan.errors, &mut found);
        let playtime = load_playtime(&self.path(PLAYTIME), &mut scan.errors);

        for f in found {
            // Heroic keys its play time by the store's app name; sideload ids carry a prefix.
            let heroic_name = f
                .source_id
                .strip_prefix("heroic-sideload:")
                .unwrap_or(&f.source_id);
            if let Some((subdir, url)) = f.cover {
                scan.covers.push(CoverSource {
                    source_id: f.source_id.clone(),
                    subdir,
                    url,
                });
            }
            scan.games.push(DiscoveredGame {
                source: ImportSource::Heroic,
                source_id: f.source_id.clone(),
                name: f.name,
                install_dir: Some(f.install_dir),
                executables: f.executables,
                playtime_minutes: playtime.get(heroic_name).copied(),
                already_imported: false,
                ignored: false,
                game_source: Some(f.game_source),
            });
        }
        scan.games.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.source_id.cmp(&b.source_id))
        });
        Ok(scan)
    }
}
