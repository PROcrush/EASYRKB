//! What the history tables hold. READ-ONLY.
//!
//! The Histories section of the source rail needs a backend, and rekordbox's
//! own history is more than one table — a session list, its tracks, and the
//! folders sessions are filed under. This prints the schema of every table
//! whose name mentions history, how many rows each holds, and the newest few
//! sessions, so the reader is written against what is there rather than a
//! guess at it.
//!
//! `cargo run -p rbl-db --example history`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

fn main() {
    let db = match rbl_db::Library::open_installed_read_only() {
        Ok(db) => db,
        Err(e) => {
            println!("cannot open library: {e}");
            return;
        }
    };
    let conn = db.connection();

    let mut stmt = conn
        .prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table'
             AND lower(name) LIKE '%history%' ORDER BY name",
        )
        .unwrap();
    let tables: Vec<String> =
        stmt.query_map([], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap();

    for table in &tables {
        let sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |r| r.get(0),
            )
            .unwrap();
        let rows: i64 =
            conn.query_row(&format!("SELECT COUNT(*) FROM `{table}`"), [], |r| r.get(0)).unwrap();
        let live: i64 = conn
            .query_row(
                &format!("SELECT COUNT(*) FROM `{table}` WHERE rb_local_deleted = 0"),
                [],
                |r| r.get(0),
            )
            .unwrap_or(-1);
        println!("== {table}: {rows} rows, {live} not deleted\n{sql}\n");
    }

    for table in &tables {
        println!("-- newest 5 of {table}");
        let cols: Vec<String> = conn
            .prepare(&format!("SELECT * FROM `{table}` LIMIT 0"))
            .unwrap()
            .column_names()
            .iter()
            .map(|c| (*c).to_string())
            .collect();
        let mut stmt = conn
            .prepare(&format!("SELECT * FROM `{table}` ORDER BY created_at DESC LIMIT 5"))
            .unwrap();
        let mut rows = stmt.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            let cells: Vec<String> = cols
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let value: rusqlite::types::Value = row.get(i).unwrap();
                    format!("{name}={value:?}")
                })
                .collect();
            println!("{}", cells.join(" "));
        }
        println!();
    }
}
