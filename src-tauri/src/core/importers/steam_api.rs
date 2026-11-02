//! Steam Web API `GetOwnedGames`: launcher-reported playtime for Steam games.

use rusqlite::{Connection, params};
use serde::Deserialize;

use super::store::set_imported_playtime;
use crate::core::db::repo::is_ignored_import;
use crate::core::types::{GameSource, ImportResult, ImportSource, SteamApiSettings};
use crate::error::{AppError, AppResult};

const ENDPOINT: &str = "https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/";

/// A configured key and SteamID64, trimmed. Errors say which one is missing.
pub fn credentials(settings: &SteamApiSettings) -> AppResult<(String, String)> {
    fn clean(v: &Option<String>) -> Option<&str> {
        v.as_deref().map(str::trim).filter(|v| !v.is_empty())
    }
    let key = clean(&settings.api_key)
        .ok_or_else(|| AppError::Other("Set a Steam Web API key first".into()))?;
    let id = clean(&settings.steam_id64)
        .ok_or_else(|| AppError::Other("Set your SteamID64 first".into()))?;
    if id.len() != 17 || !id.bytes().all(|b| b.is_ascii_digit()) {
        return Err(AppError::Other(
            "SteamID64 must be 17 digits (for example 76561198000000000)".into(),
        ));
    }
    Ok((key.to_string(), id.to_string()))
}

pub fn owned_games_url(key: &str, steam_id: &str) -> String {
    format!(
        "{ENDPOINT}?key={key}&steamid={steam_id}&include_appinfo=1&include_played_free_games=1&format=json"
    )
}

#[derive(Deserialize)]
struct Envelope {
    response: Option<Body>,
}

#[derive(Deserialize)]
struct Body {
    #[serde(default)]
    games: Vec<Owned>,
}

#[derive(Deserialize)]
struct Owned {
    appid: u64,
    #[serde(default)]
    playtime_forever: i64,
}

/// `(appid, minutes)` pairs. Errors when the response lists no games, which is what Steam
/// returns for a profile whose game details are private.
pub fn parse_owned_games(json: &str) -> AppResult<Vec<(String, i64)>> {
    let envelope: Envelope = serde_json::from_str(json)
        .map_err(|e| AppError::Other(format!("unexpected Steam Web API response: {e}")))?;
    let games = envelope.response.map(|b| b.games).unwrap_or_default();
    if games.is_empty() {
        return Err(AppError::Other(
            "Steam returned no games. Set the profile's \"Game details\" to Public \
             (Steam > Profile > Edit Profile > Privacy Settings) and check the SteamID64."
                .into(),
        ));
    }
    Ok(games
        .into_iter()
        .map(|g| (g.appid.to_string(), g.playtime_forever.max(0)))
        .collect())
}

/// A GET that returns `(status, body)`, or a transport error. Injectable for tests.
pub type HttpGet<'a> = dyn Fn(&str) -> Result<(u16, String), String> + 'a;

/// Calls the API.
pub fn fetch_owned_games(
    settings: &SteamApiSettings,
    get: &HttpGet,
) -> AppResult<Vec<(String, i64)>> {
    let (key, id) = credentials(settings)?;
    // The key is part of the URL; keep it out of any error text.
    let scrub = |s: String| s.replace(&key, "***");
    let (status, body) = get(&owned_games_url(&key, &id))
        .map_err(|e| AppError::Other(scrub(format!("Steam Web API request failed: {e}"))))?;
    match status {
        200 => parse_owned_games(&body),
        401 | 403 => Err(AppError::Other(format!(
            "Steam rejected the API key (HTTP {status})"
        ))),
        _ => Err(AppError::Other(format!(
            "Steam Web API returned HTTP {status}"
        ))),
    }
}

/// Writes playtime for Steam games already in the library. `updated` counts those;
/// owned games that are not in the library count as `skipped`.
pub fn apply_owned_games(
    conn: &Connection,
    owned: &[(String, i64)],
    now: i64,
) -> AppResult<ImportResult> {
    let tx = conn.unchecked_transaction()?;
    let (mut updated, mut skipped) = (0, 0);
    for (app_id, minutes) in owned {
        if is_ignored_import(&tx, GameSource::Steam, app_id)? {
            skipped += 1;
            continue;
        }
        let id: Option<i32> = {
            use rusqlite::OptionalExtension;
            tx.query_row(
                "SELECT id FROM games WHERE source = 'steam' AND source_id = ?1",
                params![app_id],
                |r| r.get(0),
            )
            .optional()?
        };
        match id {
            Some(id) => {
                set_imported_playtime(&tx, id, ImportSource::Steam, *minutes, now)?;
                updated += 1;
            }
            None => skipped += 1,
        }
    }
    tx.commit()?;
    Ok(ImportResult {
        source: ImportSource::Steam,
        discovered: owned.len() as i32,
        added: 0,
        updated,
        skipped,
        errors: Vec::new(),
    })
}
