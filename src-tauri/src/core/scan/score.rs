//! Junk filtering and scoring of executables found in a game folder. Pure functions; the
//! filesystem walk lives in the parent module.

use crate::core::types::{ExeCandidate, ScanConfidence};

/// Executables smaller than this are launchers' helpers, never the game.
pub const MIN_EXE_BYTES: u64 = 512 * 1024;
/// Candidates kept per folder.
pub const MAX_CANDIDATES: usize = 8;

const JUNK_PREFIXES: [&str; 9] = [
    "unins",
    "setup",
    "install",
    "dotnet",
    "unrealcefsubprocess",
    "beservice",
    "python",
    "java",
    "node",
];
const JUNK_CONTAINS: [&str; 6] = [
    "redist",
    "crash",
    "handler",
    "report",
    "easyanticheat",
    "uninstall",
];
const JUNK_DIRS: [&str; 7] = [
    "redist",
    "_commonredist",
    "directx",
    "vcredist",
    "dotnet",
    "easyanticheat",
    "__installer",
];
const PENALTY_WORDS: [&str; 5] = ["server", "editor", "benchmark", "tool", "config"];

/// True for installers, redistributables, crash reporters, anti-cheat and runtimes.
/// `file_name` includes the extension; the comparison ignores case. (`vc_redist*`,
/// `dxsetup*` and `*redist*` are covered by `redist` and `setup`.)
pub fn is_junk_file(file_name: &str) -> bool {
    let name = file_name.to_lowercase();
    name.contains("dxsetup")
        || JUNK_PREFIXES.iter().any(|p| name.starts_with(p))
        || JUNK_CONTAINS.iter().any(|c| name.contains(c))
}

/// Folders whose contents are never the game itself.
pub fn is_junk_dir(dir_name: &str) -> bool {
    let name = dir_name.to_lowercase();
    JUNK_DIRS.contains(&name.as_str())
}

/// An executable found by the walk, before scoring.
#[derive(Debug, Clone)]
pub struct RawExe {
    pub path: String,
    /// Folder names between the game folder and the file, outermost first.
    pub rel_dirs: Vec<String>,
    pub size: u64,
}

/// Lowercase letters and digits only.
fn squash(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// The file stem without a platform suffix, squashed for comparison.
fn exe_key(file_name: &str) -> String {
    let lower = file_name.to_lowercase();
    let stem = lower.strip_suffix(".exe").unwrap_or(&lower);
    let stem = ["-win64-shipping", "-win64", "_x64", "-x64"]
        .iter()
        .find_map(|s| stem.strip_suffix(s))
        .unwrap_or(stem);
    squash(stem)
}

/// The executable's name equals the folder name, or one starts with the other (at least
/// three characters), so `KingdomCome.exe` matches `Kingdom Come Deliverance II`.
fn name_matches(exe_key: &str, folder_key: &str) -> bool {
    if exe_key.is_empty() || folder_key.is_empty() {
        return false;
    }
    if exe_key == folder_key {
        return true;
    }
    let (short, long) = if exe_key.len() <= folder_key.len() {
        (exe_key, folder_key)
    } else {
        (folder_key, exe_key)
    };
    short.len() >= 3 && long.starts_with(short)
}

fn file_name_of(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

/// Whether the file sits in the game folder's root or in a conventional binary folder.
fn good_location(rel_dirs: &[String]) -> bool {
    let lower: Vec<String> = rel_dirs.iter().map(|d| d.to_lowercase()).collect();
    match lower.as_slice() {
        [] => true,
        [.., last] if last == "bin" || last == "x64" => true,
        [.., a, b] => a == "binaries" && b == "win64",
        _ => false,
    }
}

/// Scores `exes` for the game folder named `clean_name` (junk already removed). Returns at
/// most [`MAX_CANDIDATES`], best first.
pub fn score_executables(clean_name: &str, exes: &[RawExe]) -> Vec<ExeCandidate> {
    let folder_key = squash(clean_name);
    let largest = exes.iter().map(|e| e.size).max();
    let mut largest_taken = false;

    let mut scored: Vec<ExeCandidate> = exes
        .iter()
        .map(|exe| {
            let file_name = file_name_of(&exe.path);
            let lower = file_name.to_lowercase();
            let mut score = 0;
            let mut reasons = Vec::new();

            if name_matches(&exe_key(file_name), &folder_key) {
                score += 40;
                reasons.push("name matches folder".to_string());
            }
            if lower.ends_with("-win64-shipping.exe") {
                score += 35;
                reasons.push("Unreal shipping exe".to_string());
            }
            if good_location(&exe.rel_dirs) {
                score += 15;
                reasons.push(
                    if exe.rel_dirs.is_empty() {
                        "in the game folder"
                    } else {
                        "in a binaries folder"
                    }
                    .to_string(),
                );
            }
            if Some(exe.size) == largest && !largest_taken {
                largest_taken = true;
                score += 20;
                reasons.push("largest executable".to_string());
            }
            if lower.contains("launcher") {
                score -= 15;
                reasons.push("looks like a launcher".to_string());
            }
            if PENALTY_WORDS.iter().any(|w| lower.contains(w)) {
                score -= 25;
                reasons.push("looks like a server or tool".to_string());
            }
            ExeCandidate {
                path: exe.path.clone(),
                size_bytes: exe.size as i64,
                score,
                reasons,
            }
        })
        .collect();

    scored.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then(b.size_bytes.cmp(&a.size_bytes))
            .then_with(|| a.path.cmp(&b.path))
    });
    scored.truncate(MAX_CANDIDATES);
    scored
}

/// High: top score at least 60 and 20 ahead of the runner-up. Medium: top score at least 30.
pub fn confidence_of(sorted: &[ExeCandidate]) -> ScanConfidence {
    let Some(top) = sorted.first().map(|e| e.score) else {
        return ScanConfidence::Low;
    };
    let lead = sorted.get(1).map_or(i32::MAX, |second| top - second.score);
    if top >= 60 && lead >= 20 {
        ScanConfidence::High
    } else if top >= 30 {
        ScanConfidence::Medium
    } else {
        ScanConfidence::Low
    }
}
