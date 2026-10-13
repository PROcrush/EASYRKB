//! What the Explorer costs against the real library and the real disk. READ-ONLY.
//!
//! Opens the library's largest folder the way `open_view` does for the
//! Explorer — one directory read, a path match per file, a sort — and then
//! reads a window of loose files' tags the way `fetch_rows` does, so the
//! first-rows budget can be checked with numbers rather than hoped for.
//!
//! `cargo run --release -p rbl-index --example explorerbench [folder]`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rbl_index::folder::FolderEntry;
use rbl_index::{SortColumn, TrackSource, ViewSpec};

fn main() {
    let db = match rbl_db::Library::open_installed_read_only() {
        Ok(db) => db,
        Err(e) => {
            println!("cannot open the library: {e}");
            return;
        }
    };
    let (library, _) = rbl_index::load(&db).expect("load");
    println!("{} tracks", library.len());

    // The folder named on the command line, or the one the most tracks sit in.
    let folder: PathBuf = match std::env::args().nth(1) {
        Some(given) => PathBuf::from(given),
        None => {
            let mut by_dir: HashMap<&Path, usize> = HashMap::new();
            for row in 0..library.len() {
                if let Some(dir) = Path::new(library.folder_path.get(row)).parent() {
                    *by_dir.entry(dir).or_default() += 1;
                }
            }
            by_dir.into_iter().max_by_key(|(_, n)| *n).map(|(d, _)| d.to_owned()).unwrap_or_default()
        }
    };
    println!("folder: {}", folder.display());

    // Cold: the first folder opened pays for the path map.
    let started = Instant::now();
    let _ = library.row_for_path(&folder.join("warm.mp3"));
    println!("path map built in {:?}", started.elapsed());

    let spec = ViewSpec {
        source: TrackSource::Collection,
        sort: SortColumn::TrackNo,
        descending: false,
        query: String::new(),
        filter: rbl_index::TrackFilter::default(),
    };
    for sort in [SortColumn::TrackNo, SortColumn::Title, SortColumn::Bpm] {
        let started = Instant::now();
        let listing = rbl_devices::explorer::audio_files(&folder, 5000);
        let listed = started.elapsed();
        let files = listing.entries.into_iter().map(|e| (e.name, e.path)).collect();
        let view = library.open_folder(files, &ViewSpec { sort, ..spec.clone() });
        let known = view.entries.iter().filter(|e| matches!(e, FolderEntry::Track(_))).count();
        println!(
            "{sort:?}: {} rows ({known} in the library, {} loose, truncated={}) — listed in {listed:?}, opened in {:?}",
            view.len(),
            view.len() - known,
            view.truncated,
            started.elapsed()
        );

        if sort == SortColumn::TrackNo {
            // A first page of loose files, tags and all, as `fetch_rows` reads them.
            let started = Instant::now();
            view.read_tags_in(0, 128, std::time::Duration::from_millis(200));
            let elapsed = started.elapsed();
            let (read, unread) = view.window(0, 128).iter().fold((0, 0), |(r, u), entry| match *entry {
                FolderEntry::File(index) if view.file(index).unwrap().tags_if_read().is_some() => (r + 1, u),
                FolderEntry::File(_) => (r, u + 1),
                FolderEntry::Track(_) => (r, u),
            });
            println!("  first 128 rows: {read} loose files' tags read in {elapsed:?}, {unread} left for the next fetch");
        }
    }
}
