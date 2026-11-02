//! User-chosen cover images, stored as `<app data>/covers/custom/<gameId>.<ext>`.

use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};

pub const EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "webp"];

fn custom_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("covers").join("custom")
}

/// Deletes every `<gameId>.*` file in the custom folder.
pub fn remove(data_dir: &Path, game_id: i32) {
    let Ok(entries) = std::fs::read_dir(custom_dir(data_dir)) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let stem = path.file_stem().and_then(|s| s.to_str());
        if stem == Some(game_id.to_string().as_str()) {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Copies `source` to the game's custom cover, replacing any previous one, and returns the
/// new path.
pub fn set(data_dir: &Path, game_id: i32, source: &Path) -> AppResult<PathBuf> {
    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .filter(|e| EXTENSIONS.contains(&e.as_str()))
        .ok_or_else(|| {
            AppError::Other("the cover must be a png, jpg, jpeg or webp image".into())
        })?;
    if !source.is_file() {
        return Err(AppError::Other("the image file does not exist".into()));
    }
    let dir = custom_dir(data_dir);
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(format!("{game_id}.{ext}"));
    let same_file = match (source.canonicalize(), dest.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if !same_file {
        std::fs::copy(source, &dest)?;
    }
    // Drop a previous cover with another extension.
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let stem = path.file_stem().and_then(|s| s.to_str());
            if path != dest && stem == Some(game_id.to_string().as_str()) {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    Ok(dest)
}
