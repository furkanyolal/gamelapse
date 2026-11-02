//! Folder scan: finds likely games under the user's scan roots. Runs only when the user
//! asks (the `scan_folders` command); nothing here writes to the library, see [`add`].

pub mod add;
pub mod names;
pub mod roots;
pub mod score;

use std::path::Path;

use walkdir::WalkDir;

use crate::core::db::repo::normalize_path;
use crate::core::types::{GameExecutable, ScanCandidate, ScanRoot};
use score::{MIN_EXE_BYTES, RawExe};

/// Levels walked below a root; the root's children are level 1.
pub const MAX_DEPTH: usize = 5;

/// Junctions and symlinks are never followed.
fn is_reparse_point(entry: &walkdir::DirEntry) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        entry
            .metadata()
            .is_ok_and(|m| m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
    }
    #[cfg(not(windows))]
    {
        entry.path_is_symlink()
    }
}

/// Executables under one top-level folder, junk removed, with their location and size.
fn collect_executables(folder: &Path) -> Vec<RawExe> {
    let walker = WalkDir::new(folder)
        .max_depth(MAX_DEPTH - 1)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| {
            e.depth() == 0
                || !(is_reparse_point(e)
                    || (e.file_type().is_dir()
                        && score::is_junk_dir(&e.file_name().to_string_lossy())))
        });
    let mut found = Vec::new();
    for entry in walker.flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        let is_exe = Path::new(name.as_ref())
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"));
        if !is_exe || score::is_junk_file(&name) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if meta.len() < MIN_EXE_BYTES {
            continue;
        }
        let rel_dirs: Vec<String> = entry
            .path()
            .strip_prefix(folder)
            .ok()
            .and_then(Path::parent)
            .map(|p| {
                p.components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        found.push(RawExe {
            path: entry.path().display().to_string(),
            rel_dirs,
            size: meta.len(),
        });
    }
    found
}

/// The first library game with an executable rule at or under `folder`.
fn existing_game_for(folder: &str, rules: &[GameExecutable]) -> Option<i32> {
    let folder = normalize_path(folder);
    let prefix = format!("{}\\", folder.trim_end_matches('\\'));
    rules
        .iter()
        .find(|r| {
            let p = normalize_path(&r.exe_path);
            p == folder || p.starts_with(&prefix)
        })
        .map(|r| r.game_id)
}

/// Scans one top-level folder; `None` when it holds no plausible game executable.
pub fn scan_folder(root_id: i32, folder: &Path, rules: &[GameExecutable]) -> Option<ScanCandidate> {
    let folder_name = folder.file_name()?.to_string_lossy().into_owned();
    let suggested_name = names::clean_name(&folder_name);
    let exes = collect_executables(folder);
    if exes.is_empty() {
        return None;
    }
    let executables = score::score_executables(&suggested_name, &exes);
    let folder = folder.display().to_string();
    Some(ScanCandidate {
        root_id,
        confidence: score::confidence_of(&executables),
        existing_game_id: existing_game_for(&folder, rules),
        folder,
        suggested_name,
        executables,
    })
}

/// Walks each root and returns one candidate per top-level folder that has executables,
/// roots in the given order and folders sorted by name. Unreadable roots are skipped.
pub fn scan_roots(roots: &[ScanRoot], rules: &[GameExecutable]) -> Vec<ScanCandidate> {
    let mut out = Vec::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(&root.path) else {
            tracing::warn!(path = %root.path, "scan root is not readable");
            continue;
        };
        let mut folders: Vec<_> = entries
            .flatten()
            .filter(|e| {
                e.file_type().is_ok_and(|t| t.is_dir())
                    && !is_reparse_entry(e)
                    && !score::is_junk_dir(&e.file_name().to_string_lossy())
            })
            .map(|e| e.path())
            .collect();
        folders.sort_by_key(|p| p.file_name().map(|n| n.to_ascii_lowercase()));
        out.extend(
            folders
                .iter()
                .filter_map(|f| scan_folder(root.id, f, rules)),
        );
    }
    out
}

fn is_reparse_entry(entry: &std::fs::DirEntry) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        entry
            .metadata()
            .is_ok_and(|m| m.file_attributes() & 0x400 != 0)
    }
    #[cfg(not(windows))]
    {
        entry.file_type().is_ok_and(|t| t.is_symlink())
    }
}
