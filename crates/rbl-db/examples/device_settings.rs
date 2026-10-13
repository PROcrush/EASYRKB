//! What the library holds about exported devices: `djmdDevice`, and the
//! category / sort / menu-item tables the device panel's tabs are drawn from.
//! READ-ONLY.
//!
//! `cargo run -p rbl-db --example device_settings`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use rusqlite::types::ValueRef;

fn dump(conn: &rusqlite::Connection, sql: &str) {
    println!("== {sql}");
    let mut stmt = match conn.prepare(sql) {
        Ok(stmt) => stmt,
        Err(e) => {
            println!("  FAILED: {e}\n");
            return;
        }
    };
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

fn main() {
    let db = match rbl_db::Library::open_installed_read_only() {
        Ok(db) => db,
        Err(e) => {
            println!("cannot open library: {e}");
            return;
        }
    };
    let conn = db.connection();
    for table in ["djmdDevice", "djmdCategory", "djmdSort", "djmdMenuItems", "djmdColor"] {
        dump(
            conn,
            &format!("SELECT sql FROM sqlite_master WHERE type = 'table' AND name = '{table}'"),
        );
        dump(conn, &format!("SELECT * FROM {table} WHERE rb_local_deleted = 0"));
    }
}
