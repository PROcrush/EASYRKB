//! Prints every smart playlist's rule XML, and a few facts the export needs.
//! READ-ONLY.
//!
//! `cargo run -p rbl-db --example smartlists`
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
        .prepare("SELECT ID, Name, Attribute, ParentID, SmartList FROM djmdPlaylist WHERE rb_local_deleted = 0 AND SmartList IS NOT NULL AND SmartList != '' ORDER BY Attribute, Seq")
        .unwrap();
    let mut rows = stmt.query([]).unwrap();
    while let Ok(Some(row)) = rows.next() {
        let id: String = row.get(0).unwrap();
        let name: String = row.get(1).unwrap();
        let attribute: i64 = row.get(2).unwrap();
        let parent: String = row.get(3).unwrap();
        let xml: String = row.get(4).unwrap();
        println!("== {id} {name:?} attribute={attribute} parent={parent}\n{xml}\n");
    }

    let mut stmt = conn.prepare("SELECT Attribute, COUNT(*) FROM djmdPlaylist WHERE rb_local_deleted = 0 GROUP BY Attribute").unwrap();
    let mut rows = stmt.query([]).unwrap();
    while let Ok(Some(row)) = rows.next() {
        let a: i64 = row.get(0).unwrap();
        let n: i64 = row.get(1).unwrap();
        println!("attribute {a}: {n} playlists");
    }
}
