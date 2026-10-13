//! What building the link's export plan costs on the installed library. READ-ONLY.
//!
//! `cargo run --release -p rbl-link --example exportsbench`
#![allow(
    clippy::pedantic,
    clippy::print_stdout,
    clippy::unwrap_used,
    clippy::expect_used
)]

use std::time::Instant;

fn main() {
    let db = match rbl_db::Library::open_installed_read_only() {
        Ok(db) => db,
        Err(e) => {
            println!("cannot open the library: {e}");
            return;
        }
    };
    let (library, _) = rbl_index::load(&db).expect("load");
    let started = Instant::now();
    let exports = rbl_link::files::exports(&library);
    let took = started.elapsed();
    let root = exports.get("/").expect("the / export");
    println!(
        "{} tracks: export plan in {took:?}; {} nodes up front",
        library.len(),
        root.len()
    );
    let started = Instant::now();
    let top: Vec<String> = root
        .children(root.root())
        .iter()
        .filter_map(|&i| root.name(i))
        .collect();
    println!("root lists {top:?} in {:?}", started.elapsed());
}
