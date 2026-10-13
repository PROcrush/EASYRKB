//! Opens the installed library READ-ONLY and prints what it finds.
//! Safe to run while rekordbox is in use: `cargo run -p rbl-db --example probe`
#![allow(clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

fn main() {
    match rbl_db::Library::open_installed_read_only() {
        Ok(lib) => {
            let loc = lib.location();
            println!("master.db   {}", loc.master_db.display());
            println!("share root  {}", loc.share_root.display());
            println!("mode        {:?}", lib.mode());
            println!("rekordbox running: {}", rbl_db::is_rekordbox_running());
            let s = lib.schema();
            println!("DBVersion   {:?}", s.db_version);
            println!("tables      {}", s.table_count);
            println!("support     {:?}", s.support);
            println!("schema supports writes: {}", s.schema_supports_writes());
            match lib.live_track_count() {
                Ok(n) => println!("live tracks {n}"),
                Err(e) => println!("live tracks FAILED: {e}"),
            }
            match lib.live_playlist_count() {
                Ok(n) => println!("live playlists {n}"),
                Err(e) => println!("live playlists FAILED: {e}"),
            }
        }
        Err(e) => println!("FAILED: {e}"),
    }
}
