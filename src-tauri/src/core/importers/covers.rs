//! Steam cover art, cached under `<app data>/covers/steam/<appid>.jpg`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::core::types::DiscoveredGame;

const WORKERS: usize = 4;
/// Give up on the rest when this many downloads in a row fail (most likely offline).
const MAX_CONSECUTIVE_FAILURES: usize = 8;

pub fn steam_cover_url(app_id: &str) -> String {
    format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/header.jpg")
}

pub fn steam_cover_path(data_dir: &Path, app_id: &str) -> PathBuf {
    data_dir
        .join("covers")
        .join("steam")
        .join(format!("{app_id}.jpg"))
}

/// Downloads a URL's bytes; the error is a short reason for the log.
pub type Downloader<'a> = dyn Fn(&str) -> Result<Vec<u8>, String> + Sync + 'a;

fn is_jpeg(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0xD8, 0xFF])
}

fn fetch_one(path: &Path, url: &str, download: &Downloader) -> Result<(), String> {
    let bytes = download(url)?;
    if !is_jpeg(&bytes) {
        return Err("response is not a JPEG".into());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let part = path.with_extension("jpg.part");
    std::fs::write(&part, &bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&part, path).map_err(|e| e.to_string())
}

/// Makes sure every Steam game has a cover file, downloading the missing ones. Returns
/// `sourceId -> absolute file path` for games whose cover now exists. Failures are logged
/// and otherwise ignored.
pub fn ensure_steam_covers(
    data_dir: &Path,
    games: &[DiscoveredGame],
    download: &Downloader,
) -> HashMap<String, String> {
    let mut covers = HashMap::new();
    let mut missing: Vec<&str> = Vec::new();
    for game in games {
        // App ids are numeric; anything else could escape the covers folder.
        let id = game.source_id.as_str();
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let path = steam_cover_path(data_dir, id);
        if path.is_file() {
            covers.insert(id.to_string(), path.display().to_string());
        } else {
            missing.push(id);
        }
    }
    if missing.is_empty() {
        return covers;
    }

    let next = AtomicUsize::new(0);
    let failures = AtomicUsize::new(0);
    let done: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..WORKERS.min(missing.len()) {
            scope.spawn(|| {
                loop {
                    if failures.load(Ordering::Relaxed) >= MAX_CONSECUTIVE_FAILURES {
                        return;
                    }
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(id) = missing.get(i) else { return };
                    let path = steam_cover_path(data_dir, id);
                    match fetch_one(&path, &steam_cover_url(id), download) {
                        Ok(()) => {
                            failures.store(0, Ordering::Relaxed);
                            done.lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner)
                                .push((id.to_string(), path.display().to_string()));
                        }
                        Err(e) => {
                            failures.fetch_add(1, Ordering::Relaxed);
                            tracing::debug!(app_id = %id, error = %e, "cover download failed");
                        }
                    }
                }
            });
        }
    });
    if failures.load(Ordering::Relaxed) >= MAX_CONSECUTIVE_FAILURES {
        tracing::warn!("cover downloads stopped after repeated failures");
    }
    covers.extend(
        done.into_inner()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    );
    covers
}

/// The real downloader: HTTP 200 only.
pub fn http_download(url: &str) -> Result<Vec<u8>, String> {
    let response = super::http::get(url)?;
    if response.status != 200 {
        return Err(format!("HTTP {}", response.status));
    }
    Ok(response.body)
}
