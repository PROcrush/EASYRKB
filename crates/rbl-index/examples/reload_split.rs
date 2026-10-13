//! Where the library load's time actually goes. READ-ONLY.
//!
//! An edit currently re-reads everything. Whether an incremental path is worth
//! building depends on how much of the load is the track columns and how much
//! is the playlist tree, so measure before restructuring anything.
//!
//! `cargo run --release -p rbl-index --example reload_split`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::time::Instant;

fn main() {
    let db = match rbl_db::Library::open_installed_read_only() {
        Ok(db) => db,
        Err(e) => {
            println!("cannot open library: {e}");
            return;
        }
    };

    // Three runs: the first pays for the page cache, the rest are the number.
    for run in 0..3 {
        let started = Instant::now();
        let (library, stats) = rbl_index::load(&db).expect("index");
        let total = started.elapsed().as_millis();
        println!(
            "run {run}: total {total} ms  (read {} ms, index {} ms) — \
             {} tracks, {} playlists, {} memberships, {} MB",
            stats.read_ms,
            stats.index_ms,
            stats.tracks,
            stats.playlists,
            stats.memberships,
            stats.heap_bytes / 1_048_576,
        );
        drop(library);
    }

    // What the incremental path actually costs, which is the number that
    // matters: a playlist edit re-reads only the tree.
    let (library, _) = rbl_index::load(&db).expect("index");
    for _ in 0..3 {
        let started = Instant::now();
        let playlists = rbl_index::reload_playlists(&db, &library).expect("playlists");
        println!(
            "incremental playlist reload: {} ms  ({} playlists)",
            started.elapsed().as_millis(),
            playlists.len()
        );
    }

    // The floor: the playlist tables read with no indexing at all.
    let conn = db.connection();
    for _ in 0..3 {
        let started = Instant::now();
        let mut stmt = conn
            .prepare("SELECT ID, Name, ParentID, Seq FROM djmdPlaylist WHERE rb_local_deleted = 0")
            .unwrap();
        let playlists = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .filter_map(Result::ok)
            .count();
        let mut stmt = conn
            .prepare(
                "SELECT PlaylistID, ContentID, TrackNo FROM djmdSongPlaylist
                 WHERE rb_local_deleted = 0",
            )
            .unwrap();
        let memberships = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .filter_map(Result::ok)
            .count();
        println!(
            "playlist tables alone: {} ms  ({playlists} playlists, {memberships} memberships)",
            started.elapsed().as_millis()
        );
    }
}
