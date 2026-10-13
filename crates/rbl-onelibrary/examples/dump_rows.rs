//! Prints the rows of the reference tables an export has to reproduce.
//! READ-ONLY.
//!
//! `cargo run -p rbl-onelibrary --example dump_rows -- <path> <table>...`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use rusqlite::types::ValueRef;

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next().map(PathBuf::from) else {
        println!("usage: dump_rows <exportLibrary.db> <table>...");
        return;
    };
    let tables: Vec<String> = args.collect();
    let db = rbl_onelibrary::ExportLibrary::open_read_only(&path).expect("open");
    let conn = db.connection();

    for table in &tables {
        println!("== {table} ==");
        let mut stmt = conn.prepare(&format!("SELECT * FROM \"{table}\"")).unwrap();
        let names: Vec<String> = stmt.column_names().iter().map(|s| (*s).to_owned()).collect();
        println!("  {}", names.join(" | "));
        let mut rows = stmt.query([]).unwrap();
        while let Ok(Some(row)) = rows.next() {
            let cells: Vec<String> = (0..names.len())
                .map(|i| match row.get_ref(i) {
                    Ok(ValueRef::Null) => "NULL".to_owned(),
                    Ok(ValueRef::Integer(v)) => v.to_string(),
                    Ok(ValueRef::Real(v)) => v.to_string(),
                    Ok(ValueRef::Text(v)) => String::from_utf8_lossy(v).into_owned(),
                    Ok(ValueRef::Blob(v)) => format!("<{} bytes>", v.len()),
                    Err(_) => "?".to_owned(),
                })
                .collect();
            println!("  {}", cells.join(" | "));
        }
        println!();
    }
}
