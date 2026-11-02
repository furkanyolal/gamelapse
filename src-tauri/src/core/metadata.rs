//! Game metadata lookup through the Steam store search, and applying a match to a game.
//! Called only from user actions (the scan dialog and the Find cover button).

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use rusqlite::Connection;
use serde_json::Value;

use crate::core::db::repo;
use crate::core::importers::covers::{Downloader, download_to, steam_cover_url};
use crate::core::types::{Game, GameUpdate, MetadataMatch};
use crate::error::{AppError, AppResult};

const SEARCH_URL: &str = "https://store.steampowered.com/api/storesearch/";
const MAX_MATCHES: usize = 5;
/// Cover downloads are limited to Steam's CDN: the URL comes from the webview.
const COVER_URL_PREFIXES: [&str; 2] = [
    "https://cdn.cloudflare.steamstatic.com/steam/apps/",
    "https://cdn.akamai.steamstatic.com/steam/apps/",
];

/// A blocking HTTP GET returning `(status, body)`; the error is a short reason.
pub trait HttpGet: Send + Sync {
    fn get(&self, url: &str) -> Result<(u16, String), String>;
}

/// The real client.
pub struct UreqHttp;

impl HttpGet for UreqHttp {
    fn get(&self, url: &str) -> Result<(u16, String), String> {
        crate::core::importers::http::get(url)
            .map(|r| (r.status, String::from_utf8_lossy(&r.body).into_owned()))
    }
}

/// Percent-encodes everything but the unreserved characters.
fn url_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3);
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub fn is_cover_url(url: &str) -> bool {
    COVER_URL_PREFIXES.iter().any(|p| url.starts_with(p))
}

pub fn lookup_cover_path(data_dir: &Path, game_id: i32) -> PathBuf {
    data_dir
        .join("covers")
        .join("lookup")
        .join(format!("{game_id}.jpg"))
}

/// Maps the store search response to at most five matches.
pub fn parse_matches(body: &str) -> AppResult<Vec<MetadataMatch>> {
    let root: Value = serde_json::from_str(body)
        .map_err(|e| AppError::Other(format!("unexpected response from the Steam store: {e}")))?;
    let items = root
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::Other("unexpected response from the Steam store".into()))?;
    Ok(items
        .iter()
        .filter_map(|item| {
            let id = item.get("id").and_then(Value::as_u64)?;
            let name = item.get("name").and_then(Value::as_str)?.trim();
            let id = u32::try_from(id).ok()?;
            (!name.is_empty()).then(|| MetadataMatch {
                steam_app_id: id,
                name: name.to_string(),
                cover_url: steam_cover_url(&id.to_string()),
            })
        })
        .take(MAX_MATCHES)
        .collect())
}

/// Searches the Steam store for `name`. A blank name has no matches; network and HTTP
/// failures are errors.
pub fn lookup_game_metadata(http: &dyn HttpGet, name: &str) -> AppResult<Vec<MetadataMatch>> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(Vec::new());
    }
    let url = format!("{SEARCH_URL}?term={}&l=english&cc=US", url_encode(name));
    let (status, body) = http
        .get(&url)
        .map_err(|e| AppError::Other(format!("could not reach the Steam store: {e}")))?;
    if status != 200 {
        return Err(AppError::Other(format!(
            "the Steam store answered HTTP {status}"
        )));
    }
    parse_matches(&body)
}

/// Downloads the match's cover to `covers/lookup/<gameId>.jpg`, points the game at it and
/// renames the game when asked. The cover URL is rebuilt from the app id, so only Steam's
/// CDN is contacted. Nothing changes when the download fails. The database lock is not held
/// while downloading.
pub fn apply_game_metadata(
    db: &Mutex<Connection>,
    data_dir: &Path,
    game_id: i32,
    found: &MetadataMatch,
    rename_game: bool,
    download: &Downloader,
) -> AppResult<Game> {
    let lock = || db.lock().unwrap_or_else(PoisonError::into_inner);
    repo::get_game(&lock(), game_id)?;
    let path = lookup_cover_path(data_dir, game_id);
    let url = steam_cover_url(&found.steam_app_id.to_string());
    download_to(&path, &url, download)
        .map_err(|e| AppError::Other(format!("could not download the cover: {e}")))?;

    let conn = lock();
    let tx = conn.unchecked_transaction()?;
    repo::set_cover_path(&tx, game_id, Some(&path.to_string_lossy()))?;
    if rename_game {
        repo::update_game(
            &tx,
            game_id,
            &GameUpdate {
                name: Some(found.name.clone()),
                hidden: None,
                afk_detection_enabled: None,
            },
        )?;
    }
    tx.commit()?;
    repo::get_game(&conn, game_id)
}
