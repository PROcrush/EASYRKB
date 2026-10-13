//! Prints the shape of rows rekordbox itself wrote. READ-ONLY.
//!
//! The writer has to produce rows indistinguishable from these. Reading a real
//! one is how the conventions get settled — id format, `Attribute`, the
//! timestamp format, what `usn` versus `rb_local_usn` hold — rather than
//! inferred from the column types.
//!
//! `cargo run -p rbl-db --example row_shapes`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use rusqlite::types::ValueRef;

fn show(conn: &rusqlite::Connection, label: &str, sql: &str) {
    println!("== {label} ==");
    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(e) => {
            println!("  query failed: {e}\n");
            return;
        }
    };
    let names: Vec<String> = stmt.column_names().iter().map(|s| (*s).to_owned()).collect();
    let mut rows = stmt.query([]).unwrap();
    let mut shown = 0;
    while let Ok(Some(row)) = rows.next() {
        for (i, name) in names.iter().enumerate() {
            let value = match row.get_ref(i) {
                Ok(ValueRef::Null) => "NULL".to_owned(),
                Ok(ValueRef::Integer(v)) => format!("{v}"),
                Ok(ValueRef::Real(v)) => format!("{v}"),
                Ok(ValueRef::Text(v)) => format!("{:?}", String::from_utf8_lossy(v)),
                Ok(ValueRef::Blob(v)) => format!("<{} bytes>", v.len()),
                Err(_) => "?".to_owned(),
            };
            println!("  {name:<24} {value}");
        }
        println!();
        shown += 1;
        if shown >= 2 {
            break;
        }
    }
    if shown == 0 {
        println!("  (no rows)\n");
    }
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

    show(conn, "a playlist", "SELECT * FROM djmdPlaylist WHERE rb_local_deleted = 0 AND ParentID != 'root' LIMIT 2");
    show(conn, "a folder", "SELECT * FROM djmdPlaylist WHERE rb_local_deleted = 0 AND Attribute = 1 LIMIT 2");
    show(conn, "a playlist membership", "SELECT * FROM djmdSongPlaylist WHERE rb_local_deleted = 0 LIMIT 2");
    show(conn, "agentRegistry rows that look like counters",
         "SELECT * FROM agentRegistry WHERE int_1 IS NOT NULL OR int_2 IS NOT NULL LIMIT 6");

    // The bounds a new id has to respect.
    for (label, sql) in [
        ("distinct playlist Attribute values",
         "SELECT Attribute, COUNT(*) FROM djmdPlaylist WHERE rb_local_deleted = 0 GROUP BY Attribute"),
        ("ParentID of top-level playlists",
         "SELECT ParentID, COUNT(*) FROM djmdPlaylist WHERE rb_local_deleted = 0 GROUP BY ParentID ORDER BY COUNT(*) DESC LIMIT 3"),
        ("max rb_local_usn across the tables we write",
         "SELECT MAX(u) FROM (SELECT MAX(rb_local_usn) u FROM djmdContent UNION ALL SELECT MAX(rb_local_usn) FROM djmdPlaylist UNION ALL SELECT MAX(rb_local_usn) FROM djmdSongPlaylist)"),
        ("content id length and range",
         "SELECT MIN(LENGTH(ID)), MAX(LENGTH(ID)), MIN(CAST(ID AS INTEGER)), MAX(CAST(ID AS INTEGER)) FROM djmdContent"),
        ("playlist id length and range",
         "SELECT MIN(LENGTH(ID)), MAX(LENGTH(ID)), MIN(CAST(ID AS INTEGER)), MAX(CAST(ID AS INTEGER)) FROM djmdPlaylist"),
        ("how many playlists have usn set vs rb_local_usn",
         "SELECT COUNT(usn), COUNT(rb_local_usn), COUNT(*) FROM djmdPlaylist"),
        ("playlist rb_data_status by Attribute (0=playlist, 1=folder)",
         "SELECT Attribute, rb_data_status, COUNT(*) FROM djmdPlaylist GROUP BY Attribute, rb_data_status"),
        ("locally-created playlists (usn IS NULL): status, deleted, synced",
         "SELECT Attribute, rb_data_status, rb_local_data_status, rb_local_synced, COUNT(*) \
          FROM djmdPlaylist WHERE usn IS NULL GROUP BY 1,2,3,4"),
        ("membership rb_data_status, and how many are local-only",
         "SELECT rb_data_status, COUNT(*), SUM(usn IS NULL) FROM djmdSongPlaylist GROUP BY 1"),
        ("soft-deleted rows: what else changes",
         "SELECT rb_local_deleted, rb_data_status, rb_local_data_status, COUNT(*) \
          FROM djmdPlaylist WHERE rb_local_deleted != 0 GROUP BY 1,2,3"),
        ("are TrackNo values contiguous from 1 within a playlist?",
         "SELECT PlaylistID, COUNT(*), MIN(TrackNo), MAX(TrackNo) FROM djmdSongPlaylist \
          WHERE rb_local_deleted = 0 GROUP BY PlaylistID HAVING MAX(TrackNo) != COUNT(*) LIMIT 5"),
        ("Seq within a parent: contiguous from 0 or 1?",
         "SELECT ParentID, COUNT(*), MIN(Seq), MAX(Seq) FROM djmdPlaylist \
          WHERE rb_local_deleted = 0 GROUP BY ParentID ORDER BY COUNT(*) DESC LIMIT 4"),
        ("timestamp offsets in use",
         "SELECT SUBSTR(updated_at, -6), COUNT(*) FROM djmdPlaylist GROUP BY 1"),
    ] {
        println!("== {label} ==");
        let mut stmt = conn.prepare(sql).unwrap();
        let mut rows = stmt.query([]).unwrap();
        while let Ok(Some(row)) = rows.next() {
            let cols = row.as_ref().column_count();
            let cells: Vec<String> = (0..cols)
                .map(|i| match row.get_ref(i) {
                    Ok(ValueRef::Null) => "NULL".to_owned(),
                    Ok(ValueRef::Integer(v)) => format!("{v}"),
                    Ok(ValueRef::Text(v)) => format!("{:?}", String::from_utf8_lossy(v)),
                    other => format!("{other:?}"),
                })
                .collect();
            println!("  {}", cells.join("  |  "));
        }
        println!();
    }
}

// Appended: the population statistics that settle what a locally-created row
// looks like. A row with `usn` NULL was made on this machine and never synced,
// which is exactly what our writer produces.
