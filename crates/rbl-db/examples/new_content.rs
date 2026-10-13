//! What a locally-created `djmdContent` row looks like. READ-ONLY.
//!
//! A row with `usn IS NULL` was made on this machine and never synced — the
//! same shape an import would have to produce.
//!
//! `cargo run -p rbl-db --example new_content`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use rusqlite::types::ValueRef;

fn main() {
    let db = match rbl_db::Library::open_installed_read_only() {
        Ok(db) => db,
        Err(e) => {
            println!("cannot open library: {e}");
            return;
        }
    };
    let conn = db.connection();

    for (label, sql) in [
        ("locally-created tracks (usn IS NULL): how many",
         "SELECT COUNT(*) FROM djmdContent WHERE usn IS NULL AND rb_local_deleted = 0"),
        ("their rb_data_status / rb_local_data_status / rb_local_synced",
         "SELECT rb_data_status, rb_local_data_status, rb_local_synced, COUNT(*) \
          FROM djmdContent WHERE usn IS NULL GROUP BY 1,2,3 ORDER BY COUNT(*) DESC LIMIT 6"),
        ("Analysed on locally-created tracks",
         "SELECT Analysed, COUNT(*) FROM djmdContent WHERE usn IS NULL GROUP BY 1 ORDER BY COUNT(*) DESC LIMIT 6"),
        ("Analysed across the whole library",
         "SELECT Analysed, COUNT(*) FROM djmdContent WHERE rb_local_deleted = 0 GROUP BY 1 ORDER BY COUNT(*) DESC LIMIT 8"),
        ("Analysed where there is no analysis file",
         "SELECT Analysed, COUNT(*) FROM djmdContent WHERE rb_local_deleted = 0 \
          AND (AnalysisDataPath IS NULL OR AnalysisDataPath = '') GROUP BY 1 ORDER BY COUNT(*) DESC LIMIT 6"),
        ("ContentLink / rb_local_synced / ServiceID on local rows",
         "SELECT ContentLink, rb_local_synced, ServiceID, COUNT(*) FROM djmdContent \
          WHERE usn IS NULL GROUP BY 1,2,3 ORDER BY COUNT(*) DESC LIMIT 6"),
    ] {
        println!("== {label} ==");
        let Ok(mut stmt) = conn.prepare(sql) else {
            println!("  (query failed)\n");
            continue;
        };
        let cols = stmt.column_count();
        let mut rows = stmt.query([]).unwrap();
        while let Ok(Some(row)) = rows.next() {
            let cells: Vec<String> = (0..cols)
                .map(|i| match row.get_ref(i) {
                    Ok(ValueRef::Null) => "NULL".to_owned(),
                    Ok(ValueRef::Integer(v)) => v.to_string(),
                    Ok(ValueRef::Text(v)) => String::from_utf8_lossy(v).into_owned(),
                    other => format!("{other:?}"),
                })
                .collect();
            println!("  {}", cells.join(" | "));
        }
        println!();
    }
}
