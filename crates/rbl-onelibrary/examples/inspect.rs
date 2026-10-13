//! Dumps an `exportLibrary.db`'s tables and schema. READ-ONLY.
//!
//! `cargo run -p rbl-onelibrary --example inspect -- <path>`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

fn main() {
    let Some(path) = std::env::args().nth(1).map(PathBuf::from) else {
        println!("usage: inspect <exportLibrary.db>");
        return;
    };
    let db = match rbl_onelibrary::ExportLibrary::open_read_only(&path) {
        Ok(db) => db,
        Err(e) => {
            println!("cannot open: {e}");
            return;
        }
    };
    let tables = db.tables().expect("tables");
    println!("{} tables\n", tables.len());
    for table in &tables {
        let count = db.count(table).unwrap_or(-1);
        println!("{table:<28} {count:>6} rows");
    }
    println!();
    for table in &tables {
        if let Ok(Some(sql)) = db.schema_of(table) {
            println!("{sql};\n");
        }
    }
}
