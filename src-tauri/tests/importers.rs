mod common;

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

use common::test_db;
use gamelapse_lib::core::db::repo;
use gamelapse_lib::core::importers::epic::{self, EpicImporter};
use gamelapse_lib::core::importers::gog::{self, GogImporter};
use gamelapse_lib::core::importers::registry::{Hive, Registry};
use gamelapse_lib::core::importers::steam::{self, SteamImporter};
use gamelapse_lib::core::importers::{Importer, covers, mark_imported, steam_api, store};
use gamelapse_lib::core::types::{
    DiscoveredGame, GameSource, GameUpdate, ImportSource, MatchMode, SteamApiSettings,
};
use rusqlite::Connection;

// ------------------------------------------------------------ fixtures

fn fixture(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(rel);
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .trim_start_matches('\u{feff}')
        .to_string()
}

/// Escapes a path for use inside a VDF or JSON string.
fn esc(path: &Path) -> String {
    path.display().to_string().replace('\\', "\\\\")
}

#[derive(Default)]
struct FakeRegistry {
    strings: HashMap<(Hive, String, String), String>,
    keys: HashMap<(Hive, String), Vec<String>>,
}

impl FakeRegistry {
    fn string(mut self, hive: Hive, key: &str, name: &str, value: &str) -> Self {
        self.strings
            .insert((hive, key.into(), name.into()), value.into());
        self
    }

    fn subkeys(mut self, hive: Hive, key: &str, names: &[&str]) -> Self {
        self.keys.insert(
            (hive, key.into()),
            names.iter().map(|n| n.to_string()).collect(),
        );
        self
    }
}

impl Registry for FakeRegistry {
    fn get_string(&self, hive: Hive, key: &str, name: &str) -> Option<String> {
        self.strings
            .get(&(hive, key.to_string(), name.to_string()))
            .cloned()
    }
    fn get_u32(&self, _: Hive, _: &str, _: &str) -> Option<u32> {
        None
    }
    fn subkeys(&self, hive: Hive, key: &str) -> Option<Vec<String>> {
        self.keys.get(&(hive, key.to_string())).cloned()
    }
}

const STEAM_USER_KEY: &str = r"Software\Valve\Steam";
const STEAM_MACHINE_KEY: &str = r"SOFTWARE\WOW6432Node\Valve\Steam";

/// A Steam install on disk: `steam` with Elden Ring (plus the redistributables and a
/// manifest whose folder is gone), and a second library `lib2` with Portal 2.
struct SteamTree {
    _dir: tempfile::TempDir,
    steam: std::path::PathBuf,
    lib2: std::path::PathBuf,
}

fn steam_tree() -> SteamTree {
    let dir = tempfile::tempdir().unwrap();
    let steam = dir.path().join("Steam");
    let lib2 = dir.path().join("SteamLibrary");
    for (root, manifests) in [
        (
            &steam,
            vec![
                "appmanifest_1245620.acf",
                "appmanifest_228980.acf",
                "appmanifest_999.acf",
            ],
        ),
        (&lib2, vec!["appmanifest_620.acf"]),
    ] {
        let apps = root.join("steamapps");
        fs::create_dir_all(apps.join("common")).unwrap();
        for m in manifests {
            fs::write(apps.join(m), fixture(&format!("steam/{m}"))).unwrap();
        }
    }
    fs::create_dir_all(steam.join(r"steamapps\common\ELDEN RING")).unwrap();
    fs::create_dir_all(steam.join(r"steamapps\common\Steamworks Shared")).unwrap();
    fs::create_dir_all(lib2.join(r"steamapps\common\Portal 2")).unwrap();
    let vdf = fixture("steam/libraryfolders.vdf")
        .replace("{LIB1}", &esc(&steam))
        .replace("{LIB2}", &esc(&lib2));
    fs::write(steam.join(r"steamapps\libraryfolders.vdf"), vdf).unwrap();
    SteamTree {
        _dir: dir,
        steam,
        lib2,
    }
}

fn steam_registry(tree: &SteamTree) -> FakeRegistry {
    FakeRegistry::default().string(
        Hive::CurrentUser,
        STEAM_USER_KEY,
        "SteamPath",
        &tree.steam.display().to_string().replace('\\', "/"),
    )
}

// --------------------------------------------------------------- steam

#[test]
fn steam_path_prefers_valid_hkcu() {
    let tree = steam_tree();
    let other = tempfile::tempdir().unwrap();
    let registry = steam_registry(&tree).string(
        Hive::LocalMachine,
        STEAM_MACHINE_KEY,
        "InstallPath",
        &other.path().display().to_string(),
    );
    assert_eq!(
        steam::resolve_steam_path(&registry),
        Some(tree.steam.clone())
    );
}

#[test]
fn steam_path_falls_back_to_hklm_when_hkcu_is_stale() {
    let machine = steam_tree();
    let stale = tempfile::tempdir().unwrap(); // exists, but has no steamapps
    let registry = FakeRegistry::default()
        .string(
            Hive::CurrentUser,
            STEAM_USER_KEY,
            "SteamPath",
            &stale.path().display().to_string(),
        )
        .string(
            Hive::LocalMachine,
            STEAM_MACHINE_KEY,
            "InstallPath",
            &machine.steam.display().to_string(),
        );
    assert_eq!(
        steam::resolve_steam_path(&registry),
        Some(machine.steam.clone())
    );

    let nothing = FakeRegistry::default();
    assert_eq!(steam::resolve_steam_path(&nothing), None);
    let importer = SteamImporter::new(Box::new(nothing));
    assert!(!importer.status().available);
    assert!(importer.scan().is_err());
}

#[test]
fn parses_libraryfolders_both_formats() {
    let text = fixture("steam/libraryfolders.vdf")
        .replace("{LIB1}", r"C:\\Program Files (x86)\\Steam")
        .replace("{LIB2}", r"D:\\SteamLibrary");
    let libs = steam::parse_library_folders(&text).unwrap();
    assert_eq!(
        libs,
        vec![
            Path::new(r"C:\Program Files (x86)\Steam"),
            Path::new(r"D:\SteamLibrary")
        ]
    );

    let old = steam::parse_library_folders(&fixture("steam/libraryfolders_old.vdf")).unwrap();
    assert_eq!(
        old,
        vec![Path::new(r"D:\OldLibrary"), Path::new(r"E:\Games\Steam")]
    );
    assert!(steam::parse_library_folders("not vdf {").is_err());
}

#[test]
fn parses_app_manifest() {
    let m = steam::parse_manifest(&fixture("steam/appmanifest_1245620.acf")).unwrap();
    assert_eq!(m.app_id, "1245620");
    assert_eq!(m.name, "ELDEN RING");
    assert_eq!(m.install_dir, "ELDEN RING");
    assert_eq!(m.state_flags, Some(4));
    assert!(steam::parse_manifest("\"AppState\" { \"appid\" \"1\" }").is_err());
}

#[test]
fn steam_scan_skips_redistributables_and_missing_dirs() {
    let tree = steam_tree();
    let importer = SteamImporter::new(Box::new(steam_registry(&tree)));
    assert!(importer.status().available);
    let scan = importer.scan().unwrap();
    assert!(scan.errors.is_empty(), "{:?}", scan.errors);

    let mut ids: Vec<&str> = scan.games.iter().map(|g| g.source_id.as_str()).collect();
    ids.sort_unstable();
    assert_eq!(ids, ["1245620", "620"]);

    let elden = scan
        .games
        .iter()
        .find(|g| g.source_id == "1245620")
        .unwrap();
    assert_eq!(elden.name, "ELDEN RING");
    assert_eq!(
        elden.install_dir.as_deref(),
        Some(
            tree.steam
                .join(r"steamapps\common\ELDEN RING")
                .to_str()
                .unwrap()
        )
    );
    assert!(elden.executables.is_empty());
    assert_eq!(elden.playtime_minutes, None);
    let portal = scan.games.iter().find(|g| g.source_id == "620").unwrap();
    assert_eq!(
        portal.install_dir.as_deref(),
        Some(
            tree.lib2
                .join(r"steamapps\common\Portal 2")
                .to_str()
                .unwrap()
        )
    );
}

// ---------------------------------------------------------------- epic

fn epic_dir(install: &Path) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let manifests = dir.path().join("Manifests");
    fs::create_dir_all(&manifests).unwrap();
    for name in ["hades", "dlc", "incomplete"] {
        let text = fixture(&format!("epic/{name}.item")).replace("{INSTALL}", &esc(install));
        fs::write(manifests.join(format!("{name}.item")), text).unwrap();
    }
    fs::write(manifests.join("notes.txt"), "ignored").unwrap();
    fs::create_dir_all(install.join("Hades")).unwrap();
    dir
}

#[test]
fn epic_skips_dlc_and_incomplete_installs() {
    let games_root = tempfile::tempdir().unwrap();
    let data = epic_dir(games_root.path());
    let importer = EpicImporter::with_manifest_dir(
        Box::new(FakeRegistry::default()),
        data.path().join("Manifests"),
    );
    assert!(importer.status().available);
    let scan = importer.scan().unwrap();
    assert!(scan.errors.is_empty(), "{:?}", scan.errors);
    assert_eq!(scan.games.len(), 1);
    let hades = &scan.games[0];
    assert_eq!(hades.source, ImportSource::Epic);
    assert_eq!(hades.source_id, "Min");
    assert_eq!(hades.name, "Hades");
    let install = games_root.path().join("Hades");
    assert_eq!(
        hades.install_dir.as_deref(),
        Some(install.to_str().unwrap())
    );
    assert_eq!(
        hades.executables,
        vec![repo::normalize_path(
            install.join("Hades.exe").to_str().unwrap()
        )]
    );
}

#[test]
fn epic_skips_missing_install_dirs() {
    let games_root = tempfile::tempdir().unwrap();
    let data = epic_dir(games_root.path());
    // Same manifests, but the game folder was deleted.
    fs::remove_dir_all(games_root.path().join("Hades")).unwrap();
    let importer = EpicImporter::with_manifest_dir(
        Box::new(FakeRegistry::default()),
        data.path().join("Manifests"),
    );
    assert!(importer.scan().unwrap().games.is_empty());
}

#[test]
fn epic_falls_back_to_registry_app_data_path() {
    let games_root = tempfile::tempdir().unwrap();
    let data = epic_dir(games_root.path());
    let registry = FakeRegistry::default().string(
        Hive::LocalMachine,
        r"SOFTWARE\WOW6432Node\Epic Games\EpicGamesLauncher",
        "AppDataPath",
        &data.path().display().to_string(),
    );
    let importer =
        EpicImporter::with_manifest_dir(Box::new(registry), data.path().join("does-not-exist"));
    assert_eq!(importer.scan().unwrap().games.len(), 1);

    let none = EpicImporter::with_manifest_dir(
        Box::new(FakeRegistry::default()),
        data.path().join("does-not-exist"),
    );
    assert!(!none.status().available);
    assert!(none.scan().is_err());
}

#[test]
fn epic_bad_manifest_is_an_error_not_a_failure() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("broken.item"), "{ nope").unwrap();
    let importer =
        EpicImporter::with_manifest_dir(Box::new(FakeRegistry::default()), dir.path().into());
    let scan = importer.scan().unwrap();
    assert!(scan.games.is_empty());
    assert_eq!(scan.errors.len(), 1);
    assert!(epic::parse_manifest("{}").is_ok());
}

// ----------------------------------------------------------------- gog

const GOG_KEY: &str = r"SOFTWARE\WOW6432Node\GOG.com\Games";

fn gog_registry(install: &Path) -> FakeRegistry {
    let witcher = format!(r"{GOG_KEY}\1207658930");
    let nameless = format!(r"{GOG_KEY}\555");
    let gone = format!(r"{GOG_KEY}\888");
    FakeRegistry::default()
        .subkeys(
            Hive::LocalMachine,
            GOG_KEY,
            &["1207658930", "555", "777", "888"],
        )
        .string(Hive::LocalMachine, &witcher, "gameName", "The Witcher 3")
        .string(
            Hive::LocalMachine,
            &witcher,
            "path",
            &install.display().to_string(),
        )
        .string(
            Hive::LocalMachine,
            &witcher,
            "exe",
            &install.join(r"bin\Witcher3.exe").display().to_string(),
        )
        .string(
            Hive::LocalMachine,
            &nameless,
            "path",
            &install.display().to_string(),
        )
        .string(Hive::LocalMachine, &gone, "gameName", "Uninstalled")
        .string(Hive::LocalMachine, &gone, "path", r"C:\Nowhere\Gone")
}

fn galaxy_db(path: &Path) {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(
        "CREATE TABLE GamePieceTypes (id INTEGER PRIMARY KEY, type TEXT);
         CREATE TABLE GamePieces (releaseKey TEXT, gamePieceTypeId INTEGER, value TEXT);
         CREATE TABLE GameTimes (releaseKey TEXT, userId INTEGER, minutesInGame INTEGER);
         INSERT INTO GamePieceTypes VALUES (1, 'title'), (2, 'meta');
         INSERT INTO GamePieces VALUES
           ('gog_555', 1, '{\"title\":\"Nameless Galaxy Title\"}'),
           ('gog_1207658930', 1, '{\"title\":\"Galaxy Witcher\"}'),
           ('gog_1207658930', 2, '{}');
         INSERT INTO GameTimes VALUES ('gog_1207658930', 1, 600), ('gog_1207658930', 2, 60);",
    )
    .unwrap();
}

#[test]
fn gog_reads_registry_without_galaxy() {
    let install = tempfile::tempdir().unwrap();
    let importer = GogImporter::with_galaxy_db(
        Box::new(gog_registry(install.path())),
        install.path().join("missing.db"),
    );
    assert!(importer.status().available);
    let scan = importer.scan().unwrap();
    assert!(scan.errors.is_empty());
    // "555" has no name and no Galaxy title, "777" has no values at all, "888" has no
    // install directory on disk: all dropped.
    assert_eq!(scan.games.len(), 1);
    let g = &scan.games[0];
    assert_eq!(g.source_id, "1207658930");
    assert_eq!(g.name, "The Witcher 3");
    assert_eq!(
        g.install_dir.as_deref(),
        Some(install.path().to_str().unwrap())
    );
    assert_eq!(
        g.executables,
        vec![repo::normalize_path(
            install.path().join(r"bin\Witcher3.exe").to_str().unwrap()
        )]
    );
    assert_eq!(g.playtime_minutes, None);
}

#[test]
fn gog_adds_galaxy_playtime_and_titles() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("galaxy-2.0.db");
    galaxy_db(&db);
    let before = fs::read(&db).unwrap();

    let importer = GogImporter::with_galaxy_db(Box::new(gog_registry(dir.path())), db.clone());
    let scan = importer.scan().unwrap();
    assert!(scan.errors.is_empty(), "{:?}", scan.errors);
    let by_id: HashMap<_, _> = scan
        .games
        .iter()
        .map(|g| (g.source_id.as_str(), g))
        .collect();
    assert_eq!(by_id["1207658930"].playtime_minutes, Some(660));
    // The registry name wins; a nameless registry entry takes Galaxy's title.
    assert_eq!(by_id["1207658930"].name, "The Witcher 3");
    assert_eq!(by_id["555"].name, "Nameless Galaxy Title");
    // The original database is untouched.
    assert_eq!(fs::read(&db).unwrap(), before);

    let info = gog::read_galaxy_db(&db).unwrap();
    assert_eq!(info["1207658930"].minutes, Some(660));
}

#[test]
fn gog_schema_surprise_keeps_registry_games() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("galaxy-2.0.db");
    Connection::open(&db)
        .unwrap()
        .execute_batch("CREATE TABLE Unrelated (x INTEGER);")
        .unwrap();
    let importer = GogImporter::with_galaxy_db(Box::new(gog_registry(dir.path())), db);
    let scan = importer.scan().unwrap();
    assert_eq!(scan.games.len(), 1);
    assert_eq!(scan.errors.len(), 1);
    assert!(scan.errors[0].contains("GOG Galaxy"), "{}", scan.errors[0]);
}

#[test]
fn gog_not_installed() {
    let importer = GogImporter::with_galaxy_db(
        Box::new(FakeRegistry::default()),
        Path::new(r"C:\no\such\galaxy.db").into(),
    );
    assert!(!importer.status().available);
    assert!(importer.scan().is_err());
}

// --------------------------------------------------------------- store

fn discovered(id: &str, name: &str, dir: Option<&str>, minutes: Option<i64>) -> DiscoveredGame {
    DiscoveredGame {
        source: ImportSource::Steam,
        source_id: id.into(),
        name: name.into(),
        install_dir: dir.map(str::to_string),
        executables: vec![],
        playtime_minutes: minutes,
        already_imported: false,
    }
}

fn import(
    conn: &Connection,
    games: &[DiscoveredGame],
    covers: &HashMap<String, String>,
    now: i64,
) -> gamelapse_lib::core::types::ImportResult {
    store::import_games(conn, ImportSource::Steam, games, covers, vec![], now).unwrap()
}

fn find(conn: &Connection, id: &str) -> gamelapse_lib::core::types::Game {
    repo::list_games(conn, true)
        .unwrap()
        .into_iter()
        .find(|g| g.source_id.as_deref() == Some(id))
        .unwrap()
}

#[test]
fn import_adds_new_games_with_rules() {
    let conn = test_db();
    let mut game = discovered(
        "10",
        "Counter-Strike",
        Some(r"C:\Steam\steamapps\common\CS"),
        None,
    );
    game.executables = vec![r"c:\steam\steamapps\common\cs\cs.exe".into()];
    let result = import(&conn, &[game], &HashMap::new(), 100);
    assert_eq!(
        (
            result.discovered,
            result.added,
            result.updated,
            result.skipped
        ),
        (1, 1, 0, 0)
    );

    let g = find(&conn, "10");
    assert_eq!(g.source, GameSource::Steam);
    assert_eq!(g.sort_name, "counter-strike");
    assert_eq!(
        g.install_dir.as_deref(),
        Some(r"C:\Steam\steamapps\common\CS")
    );
    let exes = repo::list_executables(&conn, g.id).unwrap();
    assert_eq!(exes.len(), 2);
    assert!(
        exes.iter()
            .any(|e| e.match_mode == MatchMode::InstallDirPrefix
                && e.exe_path == r"c:\steam\steamapps\common\cs")
    );
    assert!(exes.iter().any(|e| e.match_mode == MatchMode::ExactPath
        && e.exe_path == r"c:\steam\steamapps\common\cs\cs.exe"));

    let scanned: i64 = conn
        .query_row(
            "SELECT last_scan_at FROM import_sources WHERE source = 'steam'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(scanned, 100);
}

#[test]
fn reimport_keeps_user_edits_and_executables() {
    let conn = test_db();
    let first = discovered(
        "10",
        "Counter-Strike",
        Some(r"C:\Steam\common\CS"),
        Some(30),
    );
    import(&conn, std::slice::from_ref(&first), &HashMap::new(), 100);

    let id = find(&conn, "10").id;
    repo::update_game(
        &conn,
        id,
        &GameUpdate {
            name: Some("My CS".into()),
            hidden: Some(true),
            afk_detection_enabled: Some(false),
        },
    )
    .unwrap();
    repo::insert_executable(&conn, id, r"D:\Mods\cs-mod.exe", MatchMode::ExactPath).unwrap();
    repo::insert_executable(
        &conn,
        id,
        r"C:\Steam\common\CS\cs.exe",
        MatchMode::ExactPath,
    )
    .unwrap();
    repo::insert_executable(&conn, id, r"cs-user.exe", MatchMode::ExeName).unwrap();

    // Unchanged data: nothing to do.
    let same = import(&conn, std::slice::from_ref(&first), &HashMap::new(), 200);
    assert_eq!((same.added, same.updated, same.skipped), (0, 0, 1));

    // The game moved and the launcher renamed it and reports more playtime.
    let moved = discovered(
        "10",
        "Counter-Strike 2",
        Some(r"D:\Steam\common\CS"),
        Some(90),
    );
    let result = import(&conn, &[moved], &HashMap::new(), 300);
    assert_eq!((result.added, result.updated, result.skipped), (0, 1, 0));

    let g = find(&conn, "10");
    assert_eq!(g.name, "My CS");
    assert!(g.hidden);
    assert!(!g.afk_detection_enabled);
    assert_eq!(g.install_dir.as_deref(), Some(r"D:\Steam\common\CS"));

    let exes: Vec<String> = repo::list_executables(&conn, id)
        .unwrap()
        .into_iter()
        .map(|e| e.exe_path)
        .collect();
    assert!(
        exes.contains(&r"d:\mods\cs-mod.exe".to_string()),
        "user rule survived"
    );
    assert!(
        exes.contains(&r"c:\steam\common\cs\cs.exe".to_string()),
        "exact rules stay"
    );
    assert!(exes.contains(&"cs-user.exe".to_string()));
    assert!(
        !exes.contains(&r"c:\steam\common\cs".to_string()),
        "old install dir rule is removed"
    );
    assert!(
        exes.contains(&r"d:\steam\common\cs".to_string()),
        "new install dir rule added"
    );
    assert_eq!(exes.len(), 4);

    let (minutes, fetched): (i64, i64) = conn
        .query_row(
            "SELECT minutes, fetched_at FROM imported_playtime WHERE game_id = ?1 AND source = 'steam'",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((minutes, fetched), (90, 300));
    let sessions: i64 = conn
        .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
        .unwrap();
    assert_eq!(sessions, 0, "launcher playtime never creates sessions");
}

#[test]
fn import_sets_cover_once_and_marks_imported() {
    let conn = test_db();
    let covers: HashMap<_, _> =
        [("10".to_string(), r"C:\data\covers\steam\10.jpg".to_string())].into();
    let game = discovered("10", "A", None, None);
    import(&conn, std::slice::from_ref(&game), &covers, 1);
    assert_eq!(
        find(&conn, "10").cover_path.as_deref(),
        Some(r"C:\data\covers\steam\10.jpg")
    );

    let mut games = vec![game, discovered("11", "B", None, None)];
    mark_imported(&conn, &mut games).unwrap();
    assert!(games[0].already_imported);
    assert!(!games[1].already_imported);
    assert_eq!(
        gamelapse_lib::core::importers::last_scan_at(&conn, ImportSource::Steam).unwrap(),
        Some(1)
    );
    assert_eq!(
        gamelapse_lib::core::importers::last_scan_at(&conn, ImportSource::Gog).unwrap(),
        None
    );
}

// -------------------------------------------------------------- covers

#[test]
fn covers_download_once_and_ignore_failures() {
    let data = tempfile::tempdir().unwrap();
    let games = vec![
        discovered("10", "A", None, None),
        discovered("11", "B", None, None),
        discovered("../evil", "C", None, None),
    ];
    let calls = Mutex::new(Vec::new());
    let download = |url: &str| -> Result<Vec<u8>, String> {
        calls.lock().unwrap().push(url.to_string());
        if url.contains("/11/") {
            Err("HTTP 404".into())
        } else {
            Ok(vec![0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3])
        }
    };
    let found = covers::ensure_steam_covers(data.path(), &games, &download);
    assert_eq!(found.len(), 1);
    let path = covers::steam_cover_path(data.path(), "10");
    assert_eq!(found["10"], path.display().to_string());
    assert!(path.is_file());
    assert!(!covers::steam_cover_path(data.path(), "11").exists());
    assert!(calls.lock().unwrap().iter().all(|u| !u.contains("evil")));

    // The existing file is reused without a download.
    calls.lock().unwrap().clear();
    let found = covers::ensure_steam_covers(data.path(), &games[..1], &download);
    assert_eq!(found.len(), 1);
    assert!(calls.lock().unwrap().is_empty());

    // Non-JPEG bodies are rejected.
    let html = |_: &str| -> Result<Vec<u8>, String> { Ok(b"<html>".to_vec()) };
    assert!(covers::ensure_steam_covers(data.path(), &games[1..2], &html).is_empty());
}

// ----------------------------------------------------------- steam api

#[test]
fn steam_api_credentials_and_errors() {
    let missing = SteamApiSettings::default();
    assert!(
        steam_api::credentials(&missing)
            .unwrap_err()
            .to_string()
            .contains("API key")
    );
    let no_id = SteamApiSettings {
        api_key: Some("KEY".into()),
        steam_id64: None,
    };
    assert!(
        steam_api::credentials(&no_id)
            .unwrap_err()
            .to_string()
            .contains("SteamID64")
    );
    let bad_id = SteamApiSettings {
        api_key: Some("KEY".into()),
        steam_id64: Some("123".into()),
    };
    assert!(steam_api::credentials(&bad_id).is_err());

    let ok = SteamApiSettings {
        api_key: Some(" SECRETKEY ".into()),
        steam_id64: Some("76561198000000000".into()),
    };
    let http_fail = steam_api::fetch_owned_games(&ok, &|_| Err("dns failure for SECRETKEY".into()));
    let msg = http_fail.unwrap_err().to_string();
    assert!(
        msg.contains("request failed") && !msg.contains("SECRETKEY"),
        "{msg}"
    );

    let forbidden = steam_api::fetch_owned_games(&ok, &|_| Ok((403, String::new())));
    assert!(forbidden.unwrap_err().to_string().contains("rejected"));

    let private = steam_api::fetch_owned_games(&ok, &|_| Ok((200, r#"{"response":{}}"#.into())));
    assert!(private.unwrap_err().to_string().contains("Public"));

    let good = steam_api::fetch_owned_games(&ok, &|url| {
        assert!(url.contains("key=SECRETKEY") && url.contains("steamid=76561198000000000"));
        Ok((
            200,
            r#"{"response":{"game_count":2,"games":[{"appid":10,"playtime_forever":120},{"appid":99,"playtime_forever":5}]}}"#.into(),
        ))
    })
    .unwrap();
    assert_eq!(good, vec![("10".to_string(), 120), ("99".to_string(), 5)]);
}

#[test]
fn steam_api_playtime_updates_existing_games_only() {
    let conn = test_db();
    import(
        &conn,
        &[discovered("10", "A", None, None)],
        &HashMap::new(),
        1,
    );
    let owned = vec![("10".to_string(), 120), ("99".to_string(), 5)];
    let result = steam_api::apply_owned_games(&conn, &owned, 50).unwrap();
    assert_eq!(
        (
            result.discovered,
            result.updated,
            result.skipped,
            result.added
        ),
        (2, 1, 1, 0)
    );
    let minutes: i64 = conn
        .query_row("SELECT minutes FROM imported_playtime", [], |r| r.get(0))
        .unwrap();
    assert_eq!(minutes, 120);
    assert_eq!(repo::list_games(&conn, true).unwrap().len(), 1);
}

// ----------------------------------------------------------- live check

/// Real Steam scan on this machine: `cargo test --test importers live -- --ignored --nocapture`.
#[test]
#[ignore]
fn live_steam_scan() {
    let importer = gamelapse_lib::core::importers::importer_for(ImportSource::Steam);
    let status = importer.status();
    println!("status: {status:?}");
    let scan = importer.scan().unwrap();
    println!("errors: {:?}", scan.errors);
    for g in &scan.games {
        println!(
            "{:>9}  {}  ->  {}",
            g.source_id,
            g.name,
            g.install_dir.as_deref().unwrap_or("-")
        );
    }
    println!("{} games", scan.games.len());
    assert!(scan.games.iter().all(|g| g.source_id != "228980"));
}
