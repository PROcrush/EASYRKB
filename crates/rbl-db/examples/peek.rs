//! Read-only: prints a fixture database's ratings, comments and playlists.
//! `cargo run -q -p rbl-db --example peek -- <master.db>`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

fn main() {
    let path = std::env::args().nth(1).expect("path to master.db");
    let location = rbl_db::LibraryLocation {
        master_db: path.clone().into(),
        share_root: std::path::Path::new(&path).with_file_name("share"),
        passphrase: rbl_db::fixture::FIXTURE_PASSPHRASE.to_owned(),
        is_real_install: false,
    };
    let lib = rbl_db::Library::open(location, rbl_db::OpenMode::ReadOnly).expect("open");
    let conn = lib.connection();
    let mut s = conn.prepare("SELECT ID, Title, Rating, Commnt, rb_local_usn FROM djmdContent WHERE Rating <> 0 OR Commnt IS NOT NULL ORDER BY ID").unwrap();
    for r in s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, Option<String>>(3)?, r.get::<_, Option<i64>>(4)?))).unwrap().flatten() {
        println!("content {r:?}");
    }
    let mut s = conn.prepare("SELECT ID, Name, ParentID, rb_local_deleted FROM djmdPlaylist ORDER BY Seq").unwrap();
    for r in s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?))).unwrap().flatten() {
        println!("playlist {r:?}");
    }
    let mut s = conn.prepare("SELECT ID, ContentID, Kind, InMsec, rb_local_deleted FROM djmdCue").unwrap();
    for r in s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?))).unwrap().flatten() {
        println!("cue {r:?}");
    }
}
