//! Exercises the command layer's logic against the REAL library, read-only,
//! without a webview. `cargo run -p rbxport --example ipc_smoke`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::time::Instant;

fn main() {
    let t0 = Instant::now();
    let db = match rbl_db::Library::open_installed_read_only() {
        Ok(db) => db,
        Err(e) => { println!("cannot open library: {e}"); return; }
    };
    let (library, stats) = rbl_index::load(&db).expect("index");
    println!("loaded {} tracks in {} ms", stats.tracks, t0.elapsed().as_millis());

    // The tree the UI will render.
    let playlists = library.playlists();
    let mut folders = 0;
    let mut leaves = 0;
    for i in 0..playlists.len() {
        if playlists.members.get(i).map_or(0, Vec::len) > 0 { leaves += 1; } else { folders += 1; }
    }
    println!("playlists: {} with tracks, {} empty/folders", leaves, folders);

    // A page of rows, as fetch_rows would build it.
    let spec = rbl_index::ViewSpec {
        source: rbl_index::TrackSource::Collection,
        sort: rbl_index::SortColumn::Title,
        descending: false,
        query: String::new(),
        filter: Default::default(),
    };
    let view = library.open_view(&spec);
    let t = Instant::now();
    let window = view.window(0, 64);
    let us = t.elapsed().as_micros();
    println!("first page: {} rows in {us} us", window.len());
    for &row in window.iter().take(5) {
        let i = row as usize;
        println!(
            "  {:<44} | {:<26} | {:>7} | {:>5} | {}",
            truncate(library.title.get(i), 44),
            truncate(library.artist_name(row), 26),
            format!("{:.2}", f64::from(library.bpm_x100[i]) / 100.0),
            library.key_name(row),
            library.date_added.get(i),
        );
    }

    // Estimate the JSON size of a page against the 64 KB response cap.
    let bytes: usize = window.iter().map(|&row| {
        let i = row as usize;
        library.title.get(i).len() + library.artist_name(row).len()
            + library.album_name(row).len() + library.comment.get(i).len()
            + library.genre_name(row).len() + library.label_name(row).len() + 120
    }).sum();
    println!("page payload ~{:.1} KB (cap 64 KB)  {}", bytes as f64 / 1024.0,
             if bytes < 64 * 1024 { "PASS" } else { "FAIL" });

    // Largest playlist, which is the worst case for open_view.
    let biggest = (0..playlists.len()).max_by_key(|&i| playlists.members.get(i).map_or(0, Vec::len)).unwrap_or(0);
    let t = Instant::now();
    let pv = library.open_view(&rbl_index::ViewSpec {
        source: rbl_index::TrackSource::Playlist(biggest),
        sort: rbl_index::SortColumn::Artist,
        descending: true,
        query: "mix".to_owned(),
        filter: Default::default(),
    });
    println!("\"{}\": {} of {} tracks match \"mix\", sorted desc in {} ms",
             playlists.name(biggest), pv.len(),
             playlists.members.get(biggest).map_or(0, Vec::len), t.elapsed().as_millis());
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_owned() } else { s.chars().take(n - 1).collect::<String>() + "…" }
}
