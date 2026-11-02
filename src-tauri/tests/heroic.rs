mod common;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use common::test_db;
use gamelapse_lib::core::db::repo;
use gamelapse_lib::core::importers::heroic::{self, HeroicImporter};
use gamelapse_lib::core::importers::{Importer, Scan, covers, mark_imported, store};
use gamelapse_lib::core::types::{GameSource, ImportSource, MatchMode};
use serde_json::json;

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

fn esc(path: &Path) -> String {
    path.display().to_string().replace('\\', "\\\\")
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// A fake `%APPDATA%\heroic` plus the game folders it points at.
struct HeroicTree {
    dir: tempfile::TempDir,
}

impl HeroicTree {
    fn config(&self) -> PathBuf {
        self.dir.path().join("heroic")
    }

    fn games(&self) -> PathBuf {
        self.dir.path().join("Games")
    }

    fn importer(&self) -> HeroicImporter {
        HeroicImporter::with_config_dir(self.config())
    }

    fn scan(&self) -> Scan {
        self.importer().scan().unwrap()
    }

    /// Epic via legendary only: RDR2 exists on disk, Cyberpunk (and its REDmod add-on) do not.
    fn epic_only() -> Self {
        let tree = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        fs::create_dir_all(tree.games().join("RedDeadRedemption2")).unwrap();
        let root = esc(tree.dir.path());
        let installed = fixture("heroic/legendary_installed.json").replace("@ROOT@", &root);
        let legendary = tree.config().join(r"legendaryConfig\legendary");
        write(&legendary.join("installed.json"), &installed);
        write(
            &legendary.join(r"metadata\Heather.json"),
            &fixture("heroic/legendary_metadata_Heather.json"),
        );
        write(
            &tree.config().join(r"store\timestamp.json"),
            &fixture("heroic/timestamp.json"),
        );
        tree
    }

    /// Epic plus GOG, Amazon and a sideloaded game.
    fn full() -> Self {
        let tree = Self::epic_only();
        let games = tree.games();
        let cfg = tree.config();

        let witcher = games.join("The Witcher 3");
        fs::create_dir_all(&witcher).unwrap();
        write(
            &witcher.join("goggame-1207664643.info"),
            &json!({ "playTasks": [
                { "category": "document", "path": "manual.pdf" },
                { "isPrimary": true, "category": "game", "path": "bin/x64/witcher3.exe" }
            ]})
            .to_string(),
        );
        fs::create_dir_all(games.join("Gone")).unwrap();
        fs::create_dir_all(games.join("MacGame")).unwrap();
        write(
            &cfg.join(r"gog_store\installed.json"),
            &json!({ "installed": [
                { "appName": "1207664643", "install_path": witcher, "platform": "windows" },
                { "appName": "111", "install_path": games.join("MacGame"), "platform": "osx" },
                { "appName": "222", "install_path": games.join("Missing"), "platform": "windows" },
            ]})
            .to_string(),
        );
        write(
            &cfg.join(r"gog_store\library.json"),
            &json!({ "games": [
                { "app_name": "1207664643", "title": "The Witcher 3: Wild Hunt",
                  "art_cover": "https://images.gog.com/abc_glx_vertical_cover.webp" },
            ]})
            .to_string(),
        );

        let amazon = games.join("Tomb Raider");
        fs::create_dir_all(&amazon).unwrap();
        write(
            &cfg.join(r"nile_config\nile\installed.json"),
            &json!([{ "id": "amzn1.adg.product.abc", "path": amazon, "version": "1" }]).to_string(),
        );
        write(
            &cfg.join(r"nile_config\nile\library.json"),
            &json!([{ "id": "amzn1.adg.product.abc", "product": { "title": "Tomb Raider GOTY" } }])
                .to_string(),
        );

        let side = games.join("Indie");
        fs::create_dir_all(&side).unwrap();
        write(&side.join("indie.exe"), "x");
        write(
            &cfg.join(r"sideload_apps\library.json"),
            &json!({ "games": [
                { "app_name": "Sideloaded1", "title": "Indie Thing", "runner": "sideload",
                  "folder_name": side, "is_installed": true,
                  "install": { "executable": side.join("indie.exe"), "platform": "Windows" } },
                { "app_name": "Sideloaded2", "title": "Not Installed", "is_installed": false,
                  "folder_name": side, "install": { "executable": "" } },
            ]})
            .to_string(),
        );
        tree
    }
}

fn by_id<'a>(scan: &'a Scan, id: &str) -> &'a gamelapse_lib::core::types::DiscoveredGame {
    scan.games
        .iter()
        .find(|g| g.source_id == id)
        .unwrap_or_else(|| panic!("no game {id} in {:?}", scan.games))
}

#[test]
fn epic_entries_skip_missing_folders_addons_and_other_platforms() {
    let tree = HeroicTree::epic_only();
    let scan = tree.scan();
    assert!(scan.errors.is_empty(), "{:?}", scan.errors);
    assert_eq!(scan.games.len(), 1, "{:?}", scan.games);

    let rdr2 = by_id(&scan, "Heather");
    assert_eq!(rdr2.source, ImportSource::Heroic);
    assert_eq!(rdr2.library_source(), GameSource::Epic);
    assert_eq!(rdr2.name, "Red Dead Redemption 2");
    assert_eq!(
        rdr2.install_dir.as_deref(),
        Some(tree.games().join("RedDeadRedemption2").to_str().unwrap())
    );
    assert_eq!(
        rdr2.executables,
        vec![
            tree.games()
                .join(r"RedDeadRedemption2\PlayRDR2.exe")
                .to_string_lossy()
                .to_lowercase()
        ]
    );
    // Heroic's own play time, by app name.
    assert_eq!(rdr2.playtime_minutes, Some(159));
    // Tall box art wins over the wide one.
    assert_eq!(scan.covers.len(), 1);
    assert_eq!(scan.covers[0].source_id, "Heather");
    assert_eq!(scan.covers[0].subdir, "epic");
    assert_eq!(
        scan.covers[0].url,
        "https://cdn1.epicgames.com/tall-1280x1440.jpg"
    );
}

#[test]
fn epic_cover_falls_back_to_the_wide_box() {
    let meta: serde_json::Value =
        serde_json::from_str(&fixture("heroic/legendary_metadata_boxonly.json")).unwrap();
    assert_eq!(
        heroic::epic_cover_url(&meta).as_deref(),
        Some("https://cdn1.epicgames.com/box-only.jpg")
    );
    assert_eq!(heroic::epic_cover_url(&json!({ "metadata": {} })), None);
}

#[test]
fn gog_through_heroic_reads_titles_art_and_the_primary_exe() {
    let tree = HeroicTree::full();
    let scan = tree.scan();
    let witcher = by_id(&scan, "1207664643");
    assert_eq!(witcher.library_source(), GameSource::Gog);
    assert_eq!(witcher.name, "The Witcher 3: Wild Hunt");
    assert_eq!(
        witcher.executables,
        vec![
            tree.games()
                .join("The Witcher 3")
                .join("bin/x64/witcher3.exe")
                .to_string_lossy()
                .replace('/', "\\")
                .to_lowercase()
        ]
    );
    let cover = scan
        .covers
        .iter()
        .find(|c| c.source_id == "1207664643")
        .unwrap();
    assert_eq!(cover.subdir, "gog");
    assert_eq!(
        cover.url,
        "https://images.gog.com/abc_glx_vertical_cover.jpg"
    );
    // Non-Windows and missing folders are skipped.
    assert!(
        scan.games
            .iter()
            .all(|g| g.source_id != "111" && g.source_id != "222")
    );
}

#[test]
fn amazon_through_nile_and_sideloaded_games() {
    let tree = HeroicTree::full();
    let scan = tree.scan();
    assert!(scan.errors.is_empty(), "{:?}", scan.errors);

    let tomb = by_id(&scan, "amzn1.adg.product.abc");
    assert_eq!(tomb.library_source(), GameSource::Amazon);
    assert_eq!(tomb.name, "Tomb Raider GOTY");

    let indie = by_id(&scan, "heroic-sideload:Sideloaded1");
    assert_eq!(indie.library_source(), GameSource::Manual);
    assert_eq!(indie.name, "Indie Thing");
    assert_eq!(indie.executables.len(), 1);
    assert!(indie.executables[0].ends_with(r"\indie.exe"));
    assert!(
        scan.games
            .iter()
            .all(|g| g.source_id != "heroic-sideload:Sideloaded2")
    );

    // Epic, GOG, Amazon, sideload.
    assert_eq!(scan.games.len(), 4, "{:?}", scan.games);
}

#[test]
fn status_follows_the_config_folder() {
    let tree = HeroicTree::epic_only();
    let status = tree.importer().status();
    assert!(status.available);
    assert_eq!(status.source, ImportSource::Heroic);
    assert_eq!(
        status.detail.as_deref(),
        Some(tree.config().to_str().unwrap())
    );

    let empty = tempfile::tempdir().unwrap();
    let importer = HeroicImporter::with_config_dir(empty.path().to_path_buf());
    assert!(!importer.status().available);
    assert!(importer.scan().is_err());
    let missing = HeroicImporter::with_config_dir(empty.path().join("nope"));
    assert!(!missing.status().available);
}

#[test]
fn a_broken_file_is_an_error_string_and_the_rest_still_imports() {
    let tree = HeroicTree::full();
    write(
        &tree.config().join(r"gog_store\installed.json"),
        "{ not json",
    );
    let scan = tree.scan();
    assert_eq!(scan.errors.len(), 1, "{:?}", scan.errors);
    assert!(scan.games.iter().any(|g| g.source_id == "Heather"));
}

#[test]
fn there_are_four_import_sources() {
    assert_eq!(ImportSource::ALL.len(), 4);
    assert_eq!(ImportSource::parse("heroic"), Some(ImportSource::Heroic));
}

fn jpeg(_: &str) -> Result<Vec<u8>, String> {
    Ok(vec![0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3])
}

#[test]
fn import_stores_each_game_under_its_own_source_with_covers_and_playtime() {
    let tree = HeroicTree::full();
    let scan = tree.scan();
    let data = tempfile::tempdir().unwrap();
    let cover_files = covers::ensure_url_covers(data.path(), &scan.covers, &jpeg);
    assert!(
        cover_files["Heather"].ends_with(r"covers\epic\Heather.jpg"),
        "{cover_files:?}"
    );
    assert!(Path::new(&cover_files["Heather"]).is_file());
    assert!(cover_files["1207664643"].ends_with(r"covers\gog\1207664643.jpg"));

    let conn = test_db();
    let result = store::import_games(
        &conn,
        ImportSource::Heroic,
        &scan.games,
        &cover_files,
        vec![],
        100,
    )
    .unwrap();
    assert_eq!((result.discovered, result.added), (4, 4));

    let games = repo::list_games(&conn, true).unwrap();
    let source_of = |name: &str| games.iter().find(|g| g.name == name).unwrap().source;
    assert_eq!(source_of("Red Dead Redemption 2"), GameSource::Epic);
    assert_eq!(source_of("The Witcher 3: Wild Hunt"), GameSource::Gog);
    assert_eq!(source_of("Tomb Raider GOTY"), GameSource::Amazon);
    assert_eq!(source_of("Indie Thing"), GameSource::Manual);

    let rdr2 = games
        .iter()
        .find(|g| g.source_id.as_deref() == Some("Heather"))
        .unwrap();
    assert!(rdr2.cover_path.as_deref().unwrap().ends_with("Heather.jpg"));
    let rules = repo::list_executables(&conn, rdr2.id).unwrap();
    assert_eq!(rules.len(), 2);
    assert!(
        rules
            .iter()
            .any(|r| r.match_mode == MatchMode::InstallDirPrefix)
    );
    assert!(
        rules
            .iter()
            .any(|r| r.match_mode == MatchMode::ExactPath && r.exe_path.ends_with("playrdr2.exe"))
    );

    let minutes: i64 = conn
        .query_row(
            "SELECT minutes FROM imported_playtime WHERE game_id = ?1 AND source = 'heroic'",
            [rdr2.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(minutes, 159);

    // A second run adds nothing, and the preview marks the games as imported.
    let again = store::import_games(
        &conn,
        ImportSource::Heroic,
        &scan.games,
        &cover_files,
        vec![],
        200,
    )
    .unwrap();
    assert_eq!((again.added, again.updated, again.skipped), (0, 0, 4));
    let mut preview = scan.games.clone();
    mark_imported(&conn, &mut preview).unwrap();
    assert!(preview.iter().all(|g| g.already_imported));
}

#[test]
fn a_game_the_epic_launcher_already_imported_is_not_duplicated() {
    let tree = HeroicTree::epic_only();
    let scan = tree.scan();
    let conn = test_db();
    // What the official Epic importer would have stored for the same app name.
    let mut epic = scan.games.clone();
    epic[0].source = ImportSource::Epic;
    epic[0].game_source = None;
    store::import_games(&conn, ImportSource::Epic, &epic, &HashMap::new(), vec![], 1).unwrap();

    let result = store::import_games(
        &conn,
        ImportSource::Heroic,
        &scan.games,
        &HashMap::new(),
        vec![],
        2,
    )
    .unwrap();
    assert_eq!(result.added, 0);
    assert_eq!(repo::list_games(&conn, true).unwrap().len(), 1);
}

/// Reads this PC's real Heroic folder. Run with
/// `cargo test --test heroic live_heroic -- --ignored --nocapture`.
#[test]
#[ignore]
fn live_heroic_scan() {
    let importer = HeroicImporter::new();
    println!("status: {:?}", importer.status());
    let scan = importer.scan().unwrap();
    for g in &scan.games {
        println!(
            "{:?} id={} name={:?} dir={:?} exes={:?} playtime={:?}",
            g.library_source(),
            g.source_id,
            g.name,
            g.install_dir,
            g.executables,
            g.playtime_minutes
        );
    }
    for c in &scan.covers {
        println!("cover {} -> {} ({})", c.source_id, c.url, c.subdir);
    }
    println!("errors: {:?}", scan.errors);
}
