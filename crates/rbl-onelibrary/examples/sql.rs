//! READ-ONLY: runs one SELECT against a stick's `exportLibrary.db` and
//! prints the rows pipe-separated.
//!
//! `cargo run -q -p rbl-onelibrary --example sql -- <exportLibrary.db> "SELECT * FROM sort"`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

fn main() {
    let path = std::env::args().nth(1).expect("a path to exportLibrary.db");
    let sql = std::env::args().nth(2).expect("a SELECT statement");
    let library = rbl_onelibrary::ExportLibrary::open_read_only(std::path::Path::new(&path)).expect("open");
    let conn = library.connection();
    let mut stmt = conn.prepare(&sql).expect("prepare");
    let columns: Vec<String> = stmt.column_names().iter().map(|c| (*c).to_owned()).collect();
    println!("{}", columns.join(" | "));
    let rows = stmt
        .query_map([], |r| {
            Ok((0..columns.len())
                .map(|i| match r.get_ref(i) {
                    Ok(rusqlite::types::ValueRef::Null) => String::new(),
                    Ok(rusqlite::types::ValueRef::Integer(v)) => v.to_string(),
                    Ok(rusqlite::types::ValueRef::Real(v)) => v.to_string(),
                    Ok(rusqlite::types::ValueRef::Text(t)) => String::from_utf8_lossy(t).into_owned(),
                    Ok(rusqlite::types::ValueRef::Blob(b)) => format!("<{} bytes>", b.len()),
                    Err(e) => e.to_string(),
                })
                .collect::<Vec<_>>()
                .join(" | "))
        })
        .expect("query");
    for row in rows.flatten() {
        println!("{row}");
    }
}
