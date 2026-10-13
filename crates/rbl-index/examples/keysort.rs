//! READ-ONLY: what sorting the collection by key gives on the live library.
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]
use rbl_index::{SortColumn, TrackSource, ViewSpec};

fn main() {
    let db = rbl_db::Library::open_installed_read_only().expect("open");
    let (library, _) = rbl_index::load(&db).expect("index");
    let view = library.open_view(&ViewSpec {
        source: TrackSource::Collection, sort: SortColumn::Key, descending: false, query: String::new(),
        filter: Default::default(),
    });
    let rows = view.window(0, view.len());
    let mut seen: Vec<(String, usize)> = Vec::new();
    for &r in rows {
        let k = library.key_name(r).to_owned();
        match seen.last_mut() { Some((last, n)) if *last == k => *n += 1, _ => seen.push((k, 1)) }
    }
    println!("{} runs over {} rows; first 30 runs:", seen.len(), rows.len());
    for (k, n) in seen.iter().take(30) { println!("  {k:?} x{n}"); }
    println!("keys interner order: {:?}", (0..library.keys.len()).map(|i| library.keys.name(i as u32)).collect::<Vec<_>>());
}
