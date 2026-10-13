//! Runs the writer against a **copy** of a real library, at real scale.
//!
//! The write tests use a fixture of forty tracks. This proves the same
//! operations against 38,681 real ones with the real schema, real ids and the
//! USN counter already deep into the millions — the conditions a fixture
//! cannot reproduce.
//!
//! Takes the copy's path, and refuses anything that looks like the installed
//! library so it cannot be pointed at the original by accident.
//!
//! `cargo run --release -p rbl-db --example writer_at_scale -- <copy>/master.db`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let Some(path) = std::env::args().nth(1).map(PathBuf::from) else {
        println!("usage: writer_at_scale <path to a COPY of master.db>");
        return;
    };

    // The installed library is never the target, whatever was typed.
    if let Ok(real) = rbl_db::detect() {
        if path.canonicalize().ok() == real.master_db.canonicalize().ok() {
            println!("refusing: that is the installed library, not a copy");
            return;
        }
    }

    let passphrase = match rbl_db::detect() {
        Ok(real) => real.passphrase,
        Err(e) => {
            println!("cannot read the passphrase from the install: {e}");
            return;
        }
    };
    let location = rbl_db::LibraryLocation {
        master_db: path.clone(),
        share_root: path.parent().map(|p| p.join("share")).unwrap_or_default(),
        passphrase,
        // A copy, so the write guards let it through.
        is_real_install: false,
    };

    let backups = path.parent().map(|p| p.join("backups")).unwrap_or_default();
    let mut writer = match rbl_db::write::Writer::open(location, &backups) {
        Ok(w) => w,
        Err(e) => {
            println!("cannot open the copy for writing: {e}");
            return;
        }
    };

    let before = writer.library().live_track_count().unwrap_or(0);
    let playlists_before = writer.library().live_playlist_count().unwrap_or(0);
    println!("copy holds {before} tracks, {playlists_before} playlists\n");

    // A playlist, at the scale the real tree already has.
    let t = Instant::now();
    let list = writer.create_playlist("RB-LITE-SCALE-TEST", rbl_db::write::ROOT).expect("playlist");
    println!("create_playlist        {:>6} ms", t.elapsed().as_millis());

    // Fill it with real tracks.
    let ids: Vec<String> = {
        let conn = writer.library().connection();
        let mut stmt = conn
            .prepare("SELECT ID FROM djmdContent WHERE rb_local_deleted = 0 LIMIT 500")
            .unwrap();
        stmt.query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .filter_map(Result::ok)
            .collect()
    };
    let t = Instant::now();
    let added = writer.add_tracks(&list, &ids).expect("add");
    println!("add {:>3} tracks         {:>6} ms  ({} rows)", ids.len(), t.elapsed().as_millis(), added.rows);

    let t = Instant::now();
    let mut reversed = ids.clone();
    reversed.reverse();
    writer.reorder(&list, &reversed).expect("reorder");
    println!("reorder                {:>6} ms", t.elapsed().as_millis());

    // A cue on a real track, with the shape the counting settled.
    let t = Instant::now();
    let cue = writer.add_cue(ids.first().expect("a track"), 1, 45_000).expect("cue");
    println!("add_cue                {:>6} ms", t.elapsed().as_millis());

    let t = Instant::now();
    writer.set_rating(ids.first().expect("a track"), 4).expect("rating");
    println!("set_rating             {:>6} ms", t.elapsed().as_millis());

    let t = Instant::now();
    writer.delete_playlist(&list).expect("delete");
    println!("delete_playlist        {:>6} ms", t.elapsed().as_millis());
    writer.delete_cue(&cue).expect("delete cue");

    // Nothing may have been lost along the way.
    let after = writer.library().live_track_count().unwrap_or(0);
    println!("\ntracks before {before}, after {after}");
    assert_eq!(before, after, "the writer must not have lost or gained a track");

    // And the USN counter must still lead the tables.
    let conn = writer.library().connection();
    let counter: i64 = conn
        .query_row(
            "SELECT int_1 FROM agentRegistry WHERE registry_id = 'localUpdateCount'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let highest: i64 = conn
        .query_row(
            "SELECT MAX(u) FROM (SELECT MAX(rb_local_usn) u FROM djmdContent
             UNION ALL SELECT MAX(rb_local_usn) FROM djmdPlaylist
             UNION ALL SELECT MAX(rb_local_usn) FROM djmdSongPlaylist
             UNION ALL SELECT MAX(rb_local_usn) FROM djmdCue)",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    println!("registry counter {counter}, highest row USN {highest}");
    assert!(counter >= highest, "the counter must not fall behind the rows");
    // The backup has to be a library in its own right, not just a fast copy.
    drop(writer);
    let backup = std::fs::read_dir(&backups)
        .ok()
        .and_then(|d| {
            d.filter_map(Result::ok)
                .map(|e| e.path())
                .find(|p| p.extension().is_some_and(|x| x == "db"))
        })
        .expect("a backup");
    let restored = rbl_db::Library::open(
        rbl_db::LibraryLocation {
            master_db: backup.clone(),
            share_root: PathBuf::new(),
            passphrase: rbl_db::detect().expect("install").passphrase,
            is_real_install: false,
        },
        rbl_db::OpenMode::ReadOnly,
    )
    .expect("the backup must open as a library");
    println!(
        "backup at {} opens: {} tracks",
        backup.file_name().unwrap_or_default().to_string_lossy(),
        restored.live_track_count().unwrap_or(0)
    );

    println!("\nall operations succeeded against a real-scale library");
}
