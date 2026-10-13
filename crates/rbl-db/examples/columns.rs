//! Read-only: prints columns of a table. `cargo run -p rbl-db --example columns -- djmdProperty`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

fn main() {
    let table = std::env::args().nth(1).unwrap_or_else(|| "djmdProperty".to_owned());
    let lib = rbl_db::Library::open_installed_read_only().expect("open");
    let conn = lib.connection();
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})")).expect("prepare");
    let cols: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, String>(2)?)))
        .expect("query")
        .filter_map(Result::ok)
        .collect();
    println!("{table}: {} columns", cols.len());
    for (name, ty) in &cols {
        println!("  {name} ({ty})");
    }
    if let Some((first, _)) = cols.first() {
        let sql = format!("SELECT * FROM {table} LIMIT 1");
        let mut s2 = conn.prepare(&sql).expect("prepare row");
        let names: Vec<String> = s2.column_names().iter().map(|s| (*s).to_owned()).collect();
        let row = s2.query_row([], |r| {
            let mut out = Vec::new();
            for (i, name) in names.iter().enumerate() {
                let value = r.get::<_, rusqlite::types::Value>(i).unwrap_or(rusqlite::types::Value::Null);
                out.push(format!("{name}={value:?}"));
            }
            Ok(out)
        });
        println!("first row (keyed on {first}):");
        match row {
            Ok(vals) => for v in vals { println!("  {v}"); },
            Err(e) => println!("  none ({e})"),
        }
    }
}
