//! Times building the export tree from a list of paths, one per line, the
//! way `rbl-link`'s `files::exports` does at LINK on.
//!
//!     cargo run --release -p rbl-nfs --example vfsbench -- paths.txt
#![allow(
    clippy::pedantic,
    clippy::print_stdout,
    clippy::unwrap_used,
    clippy::expect_used
)]

use std::time::Instant;

use rbl_nfs::{split_export, Vfs};

fn main() {
    let file = std::env::args()
        .nth(1)
        .expect("usage: vfsbench <paths.txt>");
    let text = std::fs::read_to_string(&file).expect("read the path list");
    let paths: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
    let started = Instant::now();
    let mut trees: Vec<Vfs> = Vec::new();
    for path in &paths {
        let Some((export, relative)) = split_export(path) else {
            continue;
        };
        let tree = trees
            .iter()
            .position(|t| t.export_name() == export)
            .unwrap_or_else(|| {
                trees.push(Vfs::new(export));
                trees.len() - 1
            });
        if let Some(tree) = trees.get_mut(tree) {
            tree.add_file_unsized(&relative, *path);
        }
    }
    println!(
        "{} paths into {} tree(s) in {:?}",
        paths.len(),
        trees.len(),
        started.elapsed()
    );
}
