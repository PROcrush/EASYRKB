//! READ-ONLY: runs one SELECT against the installed library and prints the
//! rows, pipe-separated. Safe while rekordbox is open.
//!
//! `cargo run -q -p rbl-db --example sql -- "SELECT ID, Name FROM djmdPlaylist LIMIT 5"`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use rusqlite::types::ValueRef;

fn main() {
    let sql = std::env::args().nth(1).expect("a SELECT statement");
    if !sql.trim_start().to_ascii_uppercase().starts_with("SELECT")
        && !sql.trim_start().to_ascii_uppercase().starts_with("PRAGMA")
        && !sql.trim_start().to_ascii_uppercase().starts_with("WITH")
    {
        println!("only SELECT, WITH and PRAGMA are run here");
        return;
    }
    let db = match rbl_db::Library::open_installed_read_only() {
        Ok(db) => db,
        Err(e) => {
            println!("cannot open library: {e}");
            return;
        }
    };
    let conn = db.connection();
    let mut stmt = conn.prepare(&sql).expect("prepare");
    let cols = stmt.column_count();
    let names: Vec<String> = stmt.column_names().iter().map(|s| (*s).to_owned()).collect();
    println!("{}", names.join(" | "));
    let mut rows = stmt.query([]).expect("query");
    while let Ok(Some(row)) = rows.next() {
        let cells: Vec<String> = (0..cols)
            .map(|i| match row.get_ref(i) {
                Ok(ValueRef::Null) => "NULL".to_owned(),
                Ok(ValueRef::Integer(v)) => v.to_string(),
                Ok(ValueRef::Real(v)) => v.to_string(),
                Ok(ValueRef::Text(v)) => String::from_utf8_lossy(v).into_owned(),
                Ok(ValueRef::Blob(b)) => format!("<{} bytes>", b.len()),
                Err(e) => format!("<{e}>"),
            })
            .collect();
        println!("{}", cells.join(" | "));
    }
}
