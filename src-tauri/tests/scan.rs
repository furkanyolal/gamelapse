mod common;

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use common::test_db;
use gamelapse_lib::core::db::repo::{self, normalize_path};
use gamelapse_lib::core::metadata::{self, HttpGet, UreqHttp};
use gamelapse_lib::core::scan::add::add_scanned_games;
use gamelapse_lib::core::scan::names::clean_name;
use gamelapse_lib::core::scan::roots;
use gamelapse_lib::core::scan::scan_roots;
use gamelapse_lib::core::scan::score::{
    MAX_CANDIDATES, RawExe, confidence_of, is_junk_dir, is_junk_file, score_executables,
};
use gamelapse_lib::core::types::{
    GameExecutable, GameSource, MatchMode, MetadataMatch, ScanCandidate, ScanConfidence, ScanRoot,
    ScanSelection,
};

const KB: u64 = 1024;
const MB: u64 = 1024 * KB;

fn make_exe(path: &Path, size: u64) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    File::create(path).unwrap().set_len(size).unwrap();
}

fn jpeg(_: &str) -> Result<Vec<u8>, String> {
    Ok(vec![0xFF, 0xD8, 0xFF, 0xE0, 9, 9])
}

// ---------------------------------------------------------- name cleanup

#[test]
fn name_cleanup_table() {
    let cases = [
        (
            "Kingdom-Come-Deliverance-II-AnkerGames",
            "Kingdom Come Deliverance II",
        ),
        ("Animal Well", "Animal Well"),
        ("RedDeadRedemption2", "Red Dead Redemption 2"),
        ("Cyberpunk2077", "Cyberpunk 2077"),
        ("Hollow.Knight.v1.5.78", "Hollow Knight"),
        ("Celeste [FitGirl Repack]", "Celeste"),
        ("Hades_v1.38290", "Hades"),
        ("The.Witcher.3.Wild.Hunt.1.31", "The Witcher 3 Wild Hunt"),
        ("Stardew Valley (2016)", "Stardew Valley"),
        ("Dark Souls Build 4421", "Dark Souls"),
        ("Inscryption-DODI-Repack", "Inscryption"),
        ("Hades II Multi12 Goldberg", "Hades II"),
        ("call_of_duty", "Call of Duty"),
        ("Undertale", "Undertale"),
        ("PBZ", "PBZ"),
        ("Left4Dead2", "Left 4 Dead 2"),
        ("[FitGirl]", "[FitGirl]"),
    ];
    for (raw, expected) in cases {
        assert_eq!(clean_name(raw), expected, "cleaning {raw:?}");
    }
}

// ------------------------------------------------------------ junk + score

#[test]
fn junk_file_names() {
    for junk in [
        "unins000.exe",
        "Uninstall.exe",
        "setup.exe",
        "Setup_Game.exe",
        "installer.exe",
        "vcredist_x64.exe",
        "vc_redist.x64.exe",
        "DXSETUP.exe",
        "dotnet-runtime.exe",
        "CrashReportClient.exe",
        "UnityCrashHandler64.exe",
        "BugReporter.exe",
        "UnrealCEFSubProcess.exe",
        "EasyAntiCheat_Setup.exe",
        "BEService_x64.exe",
        "python.exe",
        "java.exe",
        "node.exe",
    ] {
        assert!(is_junk_file(junk), "{junk} should be junk");
    }
    for fine in [
        "Game.exe",
        "Palworld-Win64-Shipping.exe",
        "PlayRDR2.exe",
        "Celeste.exe",
    ] {
        assert!(!is_junk_file(fine), "{fine} should not be junk");
    }
    for dir in [
        "Redist",
        "_CommonRedist",
        "DirectX",
        "vcredist",
        "DotNet",
        "EasyAntiCheat",
        "__Installer",
    ] {
        assert!(is_junk_dir(dir), "{dir}");
    }
    assert!(!is_junk_dir("Binaries"));
}

fn raw(path: &str, rel: &[&str], size: u64) -> RawExe {
    RawExe {
        path: path.to_string(),
        rel_dirs: rel.iter().map(|s| s.to_string()).collect(),
        size,
    }
}

#[test]
fn scores_follow_the_contract() {
    let exes = [
        raw(r"C:\g\Palworld.exe", &[], 700 * KB),
        raw(
            r"C:\g\Pal\Binaries\Win64\Palworld-Win64-Shipping.exe",
            &["Pal", "Binaries", "Win64"],
            9 * MB,
        ),
        raw(r"C:\g\Tools\PalworldServer.exe", &["Tools"], 2 * MB),
        raw(r"C:\g\Launcher.exe", &[], 600 * KB),
    ];
    let scored = score_executables("Palworld", &exes);
    let by = |suffix: &str| scored.iter().find(|c| c.path.ends_with(suffix)).unwrap();

    // name 40 + shipping 35 + Binaries\Win64 15 + largest 20
    assert_eq!(by("Shipping.exe").score, 110);
    // name 40 + root 15
    assert_eq!(by(r"\Palworld.exe").score, 55);
    // name 40 (folder name is a prefix) - server/tool 25, not in a good location, not largest
    assert_eq!(by("PalworldServer.exe").score, 15);
    // root 15 - launcher 15
    assert_eq!(by(r"\Launcher.exe").score, 0);

    assert_eq!(scored[0].path, exes[1].path);
    assert!(scored.windows(2).all(|w| w[0].score >= w[1].score));
    assert!(
        scored[0]
            .reasons
            .contains(&"Unreal shipping exe".to_string())
    );
    assert!(
        scored[0]
            .reasons
            .contains(&"name matches folder".to_string())
    );
    assert_eq!(confidence_of(&scored), ScanConfidence::High);
}

#[test]
fn at_most_eight_candidates() {
    let exes: Vec<RawExe> = (0..12)
        .map(|i| raw(&format!(r"C:\g\game{i}.exe"), &[], (i + 1) * MB))
        .collect();
    assert_eq!(score_executables("Whatever", &exes).len(), MAX_CANDIDATES);
}

fn cand(score: i32) -> gamelapse_lib::core::types::ExeCandidate {
    gamelapse_lib::core::types::ExeCandidate {
        path: String::new(),
        size_bytes: 0,
        score,
        reasons: vec![],
    }
}

#[test]
fn confidence_rule() {
    use ScanConfidence::*;
    assert_eq!(confidence_of(&[]), Low);
    assert_eq!(confidence_of(&[cand(60)]), High);
    assert_eq!(confidence_of(&[cand(75), cand(55)]), High);
    assert_eq!(confidence_of(&[cand(75), cand(56)]), Medium);
    assert_eq!(confidence_of(&[cand(59)]), Medium);
    assert_eq!(confidence_of(&[cand(30), cand(0)]), Medium);
    assert_eq!(confidence_of(&[cand(29)]), Low);
}

// -------------------------------------------------------------- scanning

struct Library {
    dir: tempfile::TempDir,
}

impl Library {
    fn path(&self, rel: &str) -> PathBuf {
        self.dir.path().join(rel)
    }

    fn root(&self) -> ScanRoot {
        ScanRoot {
            id: 7,
            path: self.dir.path().display().to_string(),
            created_at: 0,
        }
    }

    fn candidate(cands: &[ScanCandidate], folder: &str) -> ScanCandidate {
        cands
            .iter()
            .find(|c| c.folder.ends_with(folder))
            .unwrap_or_else(|| panic!("no candidate for {folder}: {cands:#?}"))
            .clone()
    }
}

fn library() -> Library {
    let lib = Library {
        dir: tempfile::tempdir().unwrap(),
    };
    // Plain game, plus every kind of junk.
    make_exe(&lib.path(r"Animal Well\AnimalWell.exe"), 2 * MB);
    make_exe(&lib.path(r"Animal Well\CrashHandler.exe"), 3 * MB);
    make_exe(&lib.path(r"Animal Well\unins000.exe"), 4 * MB);
    make_exe(&lib.path(r"Animal Well\tiny.exe"), 100 * KB);
    make_exe(&lib.path(r"Animal Well\redist\vcredist_x64.exe"), 5 * MB);
    make_exe(&lib.path(r"Animal Well\EasyAntiCheat\launcher.exe"), 5 * MB);
    make_exe(&lib.path(r"Animal Well\data.dll"), 6 * MB);
    // Folder name needs camel-case splitting; no exe matches it.
    make_exe(&lib.path(r"RedDeadRedemption2\RDR2.exe"), 5 * MB);
    make_exe(&lib.path(r"RedDeadRedemption2\PlayRDR2.exe"), MB);
    // Unreal layout with a launcher stub in the root.
    make_exe(&lib.path(r"Palworld\Palworld.exe"), 700 * KB);
    make_exe(
        &lib.path(r"Palworld\Pal\Binaries\Win64\Palworld-Win64-Shipping.exe"),
        9 * MB,
    );
    // Game next to its dedicated server.
    make_exe(&lib.path(r"Space Game\SpaceGame.exe"), 2 * MB);
    make_exe(&lib.path(r"Space Game\SpaceGameServer.exe"), 3 * MB);
    // Nothing recognisable.
    make_exe(&lib.path(r"Mystery\sub\deep\a.exe"), 600 * KB);
    // Depth: five levels from the root is in, six is out.
    make_exe(&lib.path(r"Edge\d1\d2\d3\game.exe"), MB);
    make_exe(&lib.path(r"TooDeep\d1\d2\d3\d4\game.exe"), MB);
    // No usable executable at all.
    make_exe(&lib.path(r"Nothing\small.exe"), 10 * KB);
    make_exe(&lib.path(r"_CommonRedist\vcredist.exe"), 5 * MB);
    // A loose file in the root is not a game folder.
    make_exe(&lib.path("loose.exe"), 5 * MB);
    lib
}

#[test]
fn scan_applies_exclusions_scores_and_confidence() {
    let lib = library();
    let cands = scan_roots(&[lib.root()], &[]);

    let mut names: Vec<_> = cands.iter().map(|c| c.suggested_name.as_str()).collect();
    names.sort();
    assert_eq!(
        names,
        [
            "Animal Well",
            "Edge",
            "Mystery",
            "Palworld",
            "Red Dead Redemption 2",
            "Space Game"
        ]
    );
    assert!(
        cands
            .iter()
            .all(|c| c.root_id == 7 && c.existing_game_id.is_none())
    );

    // Junk, tiny files, dlls and junk folders are all gone.
    let aw = Library::candidate(&cands, "Animal Well");
    assert_eq!(aw.executables.len(), 1, "{:#?}", aw.executables);
    assert!(aw.executables[0].path.ends_with("AnimalWell.exe"));
    assert_eq!(aw.executables[0].size_bytes, (2 * MB) as i64);
    // name 40 + root 15 + largest 20
    assert_eq!(aw.executables[0].score, 75);
    assert_eq!(aw.confidence, ScanConfidence::High);

    let rdr2 = Library::candidate(&cands, "RedDeadRedemption2");
    assert!(rdr2.executables[0].path.ends_with("RDR2.exe"));
    assert_eq!(rdr2.executables[0].score, 35);
    assert_eq!(rdr2.executables[1].score, 15);
    assert_eq!(rdr2.confidence, ScanConfidence::Medium);

    let pal = Library::candidate(&cands, "Palworld");
    assert!(
        pal.executables[0]
            .path
            .ends_with("Palworld-Win64-Shipping.exe")
    );
    assert_eq!(pal.executables[0].score, 110);
    assert_eq!(pal.executables.len(), 2);
    assert_eq!(pal.confidence, ScanConfidence::High);

    let space = Library::candidate(&cands, "Space Game");
    assert!(space.executables[0].path.ends_with("SpaceGame.exe"));
    assert_eq!(space.executables[0].score, 55);
    assert_eq!(space.executables[1].score, 50);
    assert_eq!(space.confidence, ScanConfidence::Medium);

    let mystery = Library::candidate(&cands, "Mystery");
    assert_eq!(mystery.executables[0].score, 20);
    assert_eq!(mystery.confidence, ScanConfidence::Low);

    // Depth 5 is scanned, depth 6 is not.
    assert_eq!(Library::candidate(&cands, "Edge").executables.len(), 1);
    assert!(cands.iter().all(|c| !c.folder.ends_with("TooDeep")));
}

#[test]
fn scan_sets_existing_game_id_from_any_rule_under_the_folder() {
    let lib = library();
    let rule = |id, game_id, path: PathBuf, mode| GameExecutable {
        id,
        game_id,
        exe_path: normalize_path(&path.to_string_lossy()),
        match_mode: mode,
    };
    let rules = [
        rule(
            1,
            41,
            lib.path(r"Animal Well\AnimalWell.exe"),
            MatchMode::ExactPath,
        ),
        rule(2, 42, lib.path("Palworld"), MatchMode::InstallDirPrefix),
        // A sibling that merely shares a name prefix does not count.
        rule(3, 43, lib.path("Space"), MatchMode::InstallDirPrefix),
        GameExecutable {
            id: 4,
            game_id: 44,
            exe_path: "mystery.exe".into(),
            match_mode: MatchMode::ExeName,
        },
    ];
    let cands = scan_roots(&[lib.root()], &rules);
    assert_eq!(
        Library::candidate(&cands, "Animal Well").existing_game_id,
        Some(41)
    );
    assert_eq!(
        Library::candidate(&cands, "Palworld").existing_game_id,
        Some(42)
    );
    assert_eq!(
        Library::candidate(&cands, "Space Game").existing_game_id,
        None
    );
    assert_eq!(Library::candidate(&cands, "Mystery").existing_game_id, None);
}

#[test]
fn scan_does_not_follow_junctions() {
    let lib = library();
    let outside = tempfile::tempdir().unwrap();
    make_exe(&outside.path().join("Linked.exe"), 8 * MB);
    let link = lib.path(r"Animal Well\link");
    let made = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(outside.path())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !made {
        eprintln!("could not create a junction here; skipping");
        return;
    }
    let cands = scan_roots(&[lib.root()], &[]);
    let aw = Library::candidate(&cands, "Animal Well");
    assert!(aw.executables.iter().all(|e| !e.path.contains("Linked")));
}

#[test]
fn unreadable_roots_are_skipped() {
    let lib = library();
    let gone = ScanRoot {
        id: 9,
        path: lib.path("does-not-exist").display().to_string(),
        created_at: 0,
    };
    let cands = scan_roots(&[gone, lib.root()], &[]);
    assert!(!cands.is_empty());
    assert!(cands.iter().all(|c| c.root_id == 7));
}

// ----------------------------------------------------------------- roots

#[test]
fn root_validation() {
    let conn = test_db();
    let dir = tempfile::tempdir().unwrap();
    let games = dir.path().join("Games");
    fs::create_dir_all(games.join(r"Steam\steamapps")).unwrap();
    fs::create_dir_all(dir.path().join("Other")).unwrap();
    fs::write(dir.path().join("file.txt"), "x").unwrap();
    let p = |rel: &str| dir.path().join(rel).display().to_string();

    let games_root = roots::add(&conn, &p("Games"), 10).unwrap();
    assert_eq!(games_root.created_at, 10);

    let err = |path: &str| roots::add(&conn, path, 11).unwrap_err().to_string();
    assert!(err(&p("file.txt")).contains("not a folder"));
    assert!(err(&p("missing")).contains("not a folder"));
    assert!(err(&p("Games")).contains("already"));
    // Different case and a trailing separator are the same folder.
    assert!(err(&format!("{}\\", p("Games").to_uppercase())).contains("already"));
    assert!(err(&p(r"Games\Steam")).contains("inside"));
    assert!(err(&p(r"Games\Steam\steamapps")).contains("inside"));
    assert!(err(&dir.path().display().to_string()).contains("contains"));

    let other = roots::add(&conn, &p("Other"), 12).unwrap();
    let listed = roots::list(&conn).unwrap();
    // Sorted by path.
    assert_eq!(
        listed.iter().map(|r| r.id).collect::<Vec<_>>(),
        [games_root.id, other.id]
    );

    // Removing a root leaves games alone.
    let game = common::add_game(&conn, "Kept", r"c:\x\k.exe", MatchMode::ExactPath);
    roots::remove(&conn, games_root.id).unwrap();
    assert_eq!(roots::list(&conn).unwrap().len(), 1);
    assert!(repo::get_game(&conn, game).is_ok());
    // The folder can be added again afterwards.
    roots::add(&conn, &p(r"Games\Steam"), 13).unwrap();
}

// ------------------------------------------------------ add_scanned_games

const COVER_URL: &str = "https://cdn.cloudflare.steamstatic.com/steam/apps/1/header.jpg";

fn selection(lib: &Library, folder: &str, exe: &str, cover: Option<&str>) -> ScanSelection {
    ScanSelection {
        folder: lib.path(folder).display().to_string(),
        name: format!("  {folder}  "),
        exe_path: lib.path(exe).display().to_string(),
        cover_url: cover.map(str::to_string),
    }
}

#[test]
fn add_scanned_games_creates_manual_games_with_two_rules_and_covers() {
    let lib = library();
    let data = tempfile::tempdir().unwrap();
    let db = Mutex::new(test_db());

    let picks = [
        selection(
            &lib,
            "Animal Well",
            r"Animal Well\AnimalWell.exe",
            Some(COVER_URL),
        ),
        // Not Steam's CDN: no download.
        selection(
            &lib,
            "Palworld",
            r"Palworld\Palworld.exe",
            Some("https://evil.example/x.jpg"),
        ),
        selection(&lib, "Edge", r"Edge\d1\d2\d3\game.exe", None),
    ];
    let result = add_scanned_games(&db, data.path(), &picks, &jpeg, 500).unwrap();
    assert_eq!(
        (
            result.discovered,
            result.added,
            result.updated,
            result.skipped
        ),
        (3, 3, 0, 0)
    );
    assert!(result.errors.is_empty(), "{:?}", result.errors);

    let conn = db.lock().unwrap();
    let games = repo::list_games(&conn, true).unwrap();
    assert_eq!(games.len(), 3);
    let aw = games.iter().find(|g| g.name == "Animal Well").unwrap();
    assert_eq!(aw.source, GameSource::Manual);
    assert_eq!(aw.source_id, None);
    assert_eq!(aw.created_at, 500);
    assert_eq!(
        aw.install_dir.as_deref(),
        Some(lib.path("Animal Well").to_str().unwrap())
    );
    let rules = repo::list_executables(&conn, aw.id).unwrap();
    assert_eq!(rules.len(), 2);
    let folder = normalize_path(&lib.path("Animal Well").to_string_lossy());
    let exe = normalize_path(&lib.path(r"Animal Well\AnimalWell.exe").to_string_lossy());
    assert!(
        rules
            .iter()
            .any(|r| r.match_mode == MatchMode::InstallDirPrefix && r.exe_path == folder)
    );
    assert!(
        rules
            .iter()
            .any(|r| r.match_mode == MatchMode::ExactPath && r.exe_path == exe)
    );

    let cover = data
        .path()
        .join("covers")
        .join("lookup")
        .join(format!("{}.jpg", aw.id));
    assert!(cover.is_file());
    assert_eq!(aw.cover_path.as_deref(), Some(cover.to_str().unwrap()));
    let pal = games.iter().find(|g| g.name == "Palworld").unwrap();
    assert_eq!(pal.cover_path, None);
}

#[test]
fn add_scanned_games_skips_attached_executables_and_reports_bad_selections() {
    let lib = library();
    let data = tempfile::tempdir().unwrap();
    let db = Mutex::new(test_db());
    let pick = selection(&lib, "Animal Well", r"Animal Well\AnimalWell.exe", None);

    add_scanned_games(&db, data.path(), std::slice::from_ref(&pick), &jpeg, 1).unwrap();
    let again = add_scanned_games(&db, data.path(), std::slice::from_ref(&pick), &jpeg, 2).unwrap();
    assert_eq!((again.added, again.skipped), (0, 1));

    let bad = [
        // Executable outside the folder.
        selection(&lib, "Palworld", r"Space Game\SpaceGame.exe", None),
        // Folder that does not exist.
        selection(&lib, "Ghost", r"Ghost\g.exe", None),
        // Blank name.
        ScanSelection {
            name: "  ".into(),
            ..selection(&lib, "Edge", r"Edge\d1\d2\d3\game.exe", None)
        },
        selection(&lib, "Space Game", r"Space Game\SpaceGame.exe", None),
    ];
    let result = add_scanned_games(&db, data.path(), &bad, &jpeg, 3).unwrap();
    assert_eq!((result.discovered, result.added), (4, 1));
    assert_eq!(result.errors.len(), 3, "{:?}", result.errors);
    assert_eq!(
        repo::list_games(&db.lock().unwrap(), true).unwrap().len(),
        2
    );
}

// -------------------------------------------------------------- metadata

struct FakeHttp {
    reply: Result<(u16, String), String>,
    seen: Mutex<Vec<String>>,
}

impl FakeHttp {
    fn new(reply: Result<(u16, String), String>) -> Self {
        Self {
            reply,
            seen: Mutex::new(vec![]),
        }
    }
}

impl HttpGet for FakeHttp {
    fn get(&self, url: &str) -> Result<(u16, String), String> {
        self.seen.lock().unwrap().push(url.to_string());
        self.reply.clone()
    }
}

const SEARCH_BODY: &str = r#"{"total":7,"items":[
  {"type":"app","name":"Undertale","id":391540,"price":{"currency":"USD"}},
  {"type":"app","name":"UNDERTALE Soundtrack","id":1,"tiny_image":"x"},
  {"type":"app","name":"Third","id":3},
  {"type":"app","name":"Fourth","id":4},
  {"type":"app","name":"Fifth","id":5},
  {"type":"app","name":"Sixth","id":6},
  {"type":"app","name":"Seventh","id":7}]}"#;

#[test]
fn metadata_search_maps_items_to_matches() {
    let http = FakeHttp::new(Ok((200, SEARCH_BODY.into())));
    let matches = metadata::lookup_game_metadata(&http, "Hollow Knight & Co").unwrap();
    assert_eq!(matches.len(), 5);
    assert_eq!(
        matches[0],
        MetadataMatch {
            steam_app_id: 391540,
            name: "Undertale".into(),
            cover_url: "https://cdn.cloudflare.steamstatic.com/steam/apps/391540/header.jpg".into(),
        }
    );
    assert_eq!(matches[4].name, "Fifth");
    let urls = http.seen.lock().unwrap();
    assert_eq!(
        urls[0],
        "https://store.steampowered.com/api/storesearch/?term=Hollow%20Knight%20%26%20Co&l=english&cc=US"
    );
}

#[test]
fn metadata_search_failures_are_errors() {
    let down = FakeHttp::new(Err("dns error".into()));
    let err = metadata::lookup_game_metadata(&down, "x")
        .unwrap_err()
        .to_string();
    assert!(err.contains("dns error"), "{err}");

    let busy = FakeHttp::new(Ok((503, String::new())));
    assert!(
        metadata::lookup_game_metadata(&busy, "x")
            .unwrap_err()
            .to_string()
            .contains("503")
    );

    let junk = FakeHttp::new(Ok((200, "<html>".into())));
    assert!(metadata::lookup_game_metadata(&junk, "x").is_err());

    let none = FakeHttp::new(Ok((200, r#"{"total":0,"items":[]}"#.into())));
    assert!(
        metadata::lookup_game_metadata(&none, "x")
            .unwrap()
            .is_empty()
    );

    // A blank name never reaches the network.
    let never = FakeHttp::new(Err("must not be called".into()));
    assert!(
        metadata::lookup_game_metadata(&never, "  ")
            .unwrap()
            .is_empty()
    );
    assert!(never.seen.lock().unwrap().is_empty());
}

fn undertale() -> MetadataMatch {
    MetadataMatch {
        steam_app_id: 391540,
        name: "Undertale".into(),
        cover_url: "ignored".into(),
    }
}

#[test]
fn apply_metadata_sets_the_cover_and_optionally_renames() {
    let data = tempfile::tempdir().unwrap();
    let db = Mutex::new(test_db());
    let id = common::add_game(
        &db.lock().unwrap(),
        "undertale-v1",
        r"c:\g\u.exe",
        MatchMode::ExactPath,
    );

    let urls = Mutex::new(Vec::new());
    let download = |url: &str| {
        urls.lock().unwrap().push(url.to_string());
        jpeg(url)
    };

    let kept = metadata::apply_game_metadata(&db, data.path(), id, &undertale(), false, &download)
        .unwrap();
    assert_eq!(kept.name, "undertale-v1");
    let cover = data
        .path()
        .join("covers")
        .join("lookup")
        .join(format!("{id}.jpg"));
    assert!(cover.is_file());
    assert_eq!(kept.cover_path.as_deref(), Some(cover.to_str().unwrap()));
    // The URL comes from the app id, not from the (untrusted) match field.
    assert_eq!(
        urls.lock().unwrap()[0],
        "https://cdn.cloudflare.steamstatic.com/steam/apps/391540/header.jpg"
    );

    let renamed =
        metadata::apply_game_metadata(&db, data.path(), id, &undertale(), true, &download).unwrap();
    assert_eq!(renamed.name, "Undertale");
    assert_eq!(renamed.sort_name, "undertale");
}

#[test]
fn apply_metadata_changes_nothing_when_the_download_fails() {
    let data = tempfile::tempdir().unwrap();
    let db = Mutex::new(test_db());
    let id = common::add_game(
        &db.lock().unwrap(),
        "Old",
        r"c:\g\o.exe",
        MatchMode::ExactPath,
    );
    let fail = |_: &str| Err("HTTP 404".to_string());
    let err = metadata::apply_game_metadata(&db, data.path(), id, &undertale(), true, &fail)
        .unwrap_err()
        .to_string();
    assert!(err.contains("404"), "{err}");
    let game = repo::get_game(&db.lock().unwrap(), id).unwrap();
    assert_eq!((game.name.as_str(), game.cover_path), ("Old", None));

    // Unknown game.
    assert!(
        metadata::apply_game_metadata(&db, data.path(), 999, &undertale(), false, &jpeg).is_err()
    );
}

// ------------------------------------------------------------ live (ignored)

/// Hits the real Steam store. Run with
/// `cargo test --test scan live_steam -- --ignored --nocapture`.
#[test]
#[ignore]
fn live_steam_lookup_undertale() {
    let matches = metadata::lookup_game_metadata(&UreqHttp, "Undertale").unwrap();
    for m in &matches {
        println!("{} {} {}", m.steam_app_id, m.name, m.cover_url);
    }
    assert!(matches.len() <= 5);
    assert!(
        matches.iter().any(|m| m.steam_app_id == 391540),
        "{matches:?}"
    );
}

/// Scans `D:\Games` (read-only). Run with
/// `cargo test --test scan live_folder -- --ignored --nocapture`.
#[test]
#[ignore]
fn live_folder_scan() {
    let root = ScanRoot {
        id: 1,
        path: r"D:\Games".into(),
        created_at: 0,
    };
    let started = std::time::Instant::now();
    let cands = scan_roots(&[root], &[]);
    println!("scanned in {:?}", started.elapsed());
    for c in &cands {
        let top = c.executables.first();
        println!(
            "{:<45} -> {:<32} {:?} top={} score={} ({} exes) {:?}",
            Path::new(&c.folder).file_name().unwrap().to_string_lossy(),
            c.suggested_name,
            c.confidence,
            top.map_or("-".into(), |e| e.path.clone()),
            top.map_or(0, |e| e.score),
            c.executables.len(),
            top.map(|e| e.reasons.clone()).unwrap_or_default()
        );
    }
}
