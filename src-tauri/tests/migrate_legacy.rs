use gamelapse_lib::core::migrate_legacy::migrate_legacy_data;

#[test]
fn copies_legacy_files_without_touching_the_source() {
    let tmp = tempfile::tempdir().unwrap();
    let old = tmp.path().join("old");
    let new = tmp.path().join("new");
    std::fs::create_dir_all(old.join("covers").join("custom")).unwrap();
    std::fs::write(old.join("gametimetracker.db"), b"db").unwrap();
    std::fs::write(old.join("settings.json"), b"{}").unwrap();
    std::fs::write(old.join("covers").join("custom").join("a.png"), b"img").unwrap();

    assert!(migrate_legacy_data(&old, &new).unwrap());
    assert_eq!(std::fs::read(new.join("gamelapse.db")).unwrap(), b"db");
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
