//! READ-ONLY: every `djmdHistory` row of the installed library as a tree, with
//! the columns rekordbox files and orders them by, so the Histories section
//! can be checked against what rekordbox draws.
//! `cargo run -q -p rbl-db --example history_rows`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

/// `ID, Name, Attribute, ParentID, Seq, DateCreated, created_at, rb_local_deleted`.
type Row = (String, String, i64, String, i64, String, String, i64);

fn main() {
    let db = rbl_db::Library::open_installed_read_only().expect("open");
    let conn = db.connection();
    let mut s = conn
        .prepare(
            "SELECT ID, Name, Attribute, ParentID, Seq, DateCreated, created_at, rb_local_deleted
             FROM djmdHistory ORDER BY ParentID, Seq",
        )
        .unwrap();
    let rows: Vec<Row> = s
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                r.get::<_, Option<i64>>(2)?.unwrap_or(-1),
                r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                r.get::<_, Option<i64>>(4)?.unwrap_or(-1),
                r.get::<_, Option<String>>(5)?.unwrap_or_default(),
                r.get::<_, Option<String>>(6)?.unwrap_or_default(),
                r.get::<_, Option<i64>>(7)?.unwrap_or(0),
            ))
        })
        .unwrap()
        .flatten()
        .collect();
    let mut by_parent: BTreeMap<String, Vec<&Row>> = BTreeMap::new();
    for row in &rows {
        by_parent.entry(row.3.clone()).or_default().push(row);
    }
    fn walk(parent: &str, depth: usize, by_parent: &BTreeMap<String, Vec<&Row>>) {
        let Some(children) = by_parent.get(parent) else { return };
        for c in children {
            println!(
                "{}{:<28} id={:<11} attr={} seq={:<3} date={:<10} created={} deleted={}",
                "  ".repeat(depth),
                c.1,
                c.0,
                c.2,
                c.4,
                c.5,
                c.6,
                c.7
            );
            walk(&c.0, depth + 1, by_parent);
        }
    }
    println!("{} rows; roots under ParentID '' or 'root':", rows.len());
    walk("", 0, &by_parent);
    walk("root", 0, &by_parent);
    let parents: Vec<&String> = by_parent.keys().collect();
    println!("\nparent ids seen: {parents:?}");
}
