use std::path::Path;

use gamelapse_lib::core::migrate_legacy::migrate_legacy_data;
use rusqlite::Connection;

/// Creates a minimal legacy database with one `games` row per cover path.
fn make_legacy_db(path: &Path, covers: &[Option<String>]) {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch("CREATE TABLE games (id INTEGER PRIMARY KEY, cover_path TEXT);")
        .unwrap();
    for (i, cover) in covers.iter().enumerate() {
        conn.execute(
            "INSERT INTO games (id, cover_path) VALUES (?1, ?2)",
            rusqlite::params![i as i64 + 1, cover],
        )
        .unwrap();
    }
}

fn covers_of(db: &Path) -> Vec<Option<String>> {
    let conn = Connection::open(db).unwrap();
    let mut stmt = conn
        .prepare("SELECT cover_path FROM games ORDER BY id")
        .unwrap();
    stmt.query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn copies_legacy_files_without_touching_the_source() {
    let tmp = tempfile::tempdir().unwrap();
    let old = tmp.path().join("old");
    let new = tmp.path().join("new");
    std::fs::create_dir_all(old.join("covers").join("custom")).unwrap();
    make_legacy_db(&old.join("gametimetracker.db"), &[]);
    std::fs::write(old.join("settings.json"), b"{}").unwrap();
    std::fs::write(old.join("covers").join("custom").join("a.png"), b"img").unwrap();

    assert!(migrate_legacy_data(&old, &new).unwrap());
    assert!(new.join("gamelapse.db").is_file());
    assert!(!new.join("gamelapse.db.migrating").exists());
    assert_eq!(std::fs::read(new.join("settings.json")).unwrap(), b"{}");
    assert!(new.join("covers").join("custom").join("a.png").is_file());
    assert!(old.join("gametimetracker.db").is_file());

    // A second run is a no-op.
    assert!(!migrate_legacy_data(&old, &new).unwrap());
}

#[test]
fn skips_when_new_db_exists_or_old_is_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let old = tmp.path().join("old");
    let new = tmp.path().join("new");
    assert!(!migrate_legacy_data(&old, &new).unwrap());

    std::fs::create_dir_all(&old).unwrap();
    std::fs::create_dir_all(&new).unwrap();
    std::fs::write(old.join("gametimetracker.db"), b"old").unwrap();
    std::fs::write(new.join("gamelapse.db"), b"new").unwrap();
    assert!(!migrate_legacy_data(&old, &new).unwrap());
    assert_eq!(std::fs::read(new.join("gamelapse.db")).unwrap(), b"new");
}

#[test]
fn rewrites_cover_paths_that_point_into_the_legacy_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let old = tmp.path().join("old");
    let new = tmp.path().join("new");
    std::fs::create_dir_all(&old).unwrap();
    let inside = old.join("covers").join("custom").join("a.png");
    let outside = tmp.path().join("elsewhere").join("b.png");
    // Same path with different letter case, as Windows treats them as equal.
    let shouty = old.to_string_lossy().to_uppercase() + "/covers/c.png";
    make_legacy_db(
        &old.join("gametimetracker.db"),
        &[
            Some(inside.to_string_lossy().into_owned()),
            Some(outside.to_string_lossy().into_owned()),
            None,
            Some(shouty),
        ],
    );

    assert!(migrate_legacy_data(&old, &new).unwrap());

    let covers = covers_of(&new.join("gamelapse.db"));
    let expected_inside = new.join("covers").join("custom").join("a.png");
    assert_eq!(
        covers[0].as_deref(),
        Some(&*expected_inside.to_string_lossy())
    );
    assert_eq!(covers[1].as_deref(), Some(&*outside.to_string_lossy()));
    assert_eq!(covers[2], None);
    let expected_shouty = new.to_string_lossy().into_owned() + "/covers/c.png";
    assert_eq!(covers[3].as_deref(), Some(expected_shouty.as_str()));

    // The legacy database keeps its original paths.
    assert_eq!(
        covers_of(&old.join("gametimetracker.db"))[0].as_deref(),
        Some(&*inside.to_string_lossy())
    );
}
