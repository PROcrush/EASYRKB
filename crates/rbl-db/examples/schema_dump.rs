//! Dumps the CREATE statements for the tables the writer touches. READ-ONLY.
//!
//! The fixture the write tests run against has to match the real schema, and
//! guessing at it is how a writer ends up producing rows rekordbox rejects.
//!
//! `cargo run -p rbl-db --example schema_dump`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

const TABLES: &[&str] = &[
    "djmdContent", "djmdPlaylist", "djmdSongPlaylist", "djmdArtist", "djmdAlbum",
    "djmdGenre", "djmdKey", "djmdLabel", "djmdColor", "djmdCue", "djmdProperty",
    "agentRegistry", "djmdMixerParam",
];

fn main() {
    let db = match rbl_db::Library::open_installed_read_only() {
        Ok(db) => db,
        Err(e) => {
            println!("cannot open library: {e}");
            return;
        }
    };
    let conn = db.connection();
    for table in TABLES {
        let sql: Option<String> = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |r| r.get(0),
            )
            .ok();
        match sql {
            Some(sql) => println!("{sql};\n"),
            None => println!("-- {table}: not present\n"),
        }
    }
}
