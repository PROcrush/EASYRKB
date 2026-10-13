//! Finding a library from an `options.json`, including one a fixture wrote.
#![allow(clippy::pedantic, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use rbl_db::fixture::{self, Shape, FIXTURE_PASSPHRASE};
use rbl_db::{detect_from, Library, OpenMode};

#[test]
fn a_fixture_s_options_json_leads_the_detector_to_the_fixture() {
    let dir = tempfile::tempdir().unwrap();
    let built = fixture::build(dir.path(), Shape::default()).unwrap();
    let options = dir.path().join("options.json");
    fixture::write_options_json(&options, &built.master_db.display().to_string(), FIXTURE_PASSPHRASE).unwrap();

    let found = detect_from(&options).expect("detect from the written file");
    assert_eq!(found.master_db, built.master_db);
    assert_eq!(found.share_root, dir.path().join("share"));
    assert_eq!(found.passphrase, FIXTURE_PASSPHRASE, "the wrapped passphrase derives back");
    assert!(found.is_real_install, "detected through options.json, it is treated as an install");

    // And the key it derived opens the database.
    let db = Library::open(found, OpenMode::ReadOnly).expect("open with the derived key");
    assert_eq!(db.schema().db_version, Some(6000));
}

#[test]
fn a_track_pointed_at_a_file_carries_its_path_name_title_and_length() {
    let dir = tempfile::tempdir().unwrap();
    let built = fixture::build(dir.path(), Shape::default()).unwrap();
    fixture::point_at_audio(&built, 2, r"C:\rig\audio\Fixture Track 2.wav", 6).unwrap();

    let db = Library::open(built, OpenMode::ReadOnly).unwrap();
    let (path, name, title, length): (String, String, String, i64) = db
        .connection()
        .query_row(
            "SELECT FolderPath, FileNameL, Title, Length FROM djmdContent WHERE ID = ?1",
            [fixture::track_id(2)],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(path, r"C:\rig\audio\Fixture Track 2.wav");
    assert_eq!(name, "Fixture Track 2.wav");
    assert_eq!(title, "Fixture Track 2");
    assert_eq!(length, 6);
}
