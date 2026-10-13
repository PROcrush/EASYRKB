//! Which colour a hot cue is drawn in. READ-ONLY.
//!
//! `djmdCue.ColorTableIndex` decides it, and `rbl-anlz`'s `cue_colours` found
//! the palette in neither the database, the skins nor the analysis files. One
//! entry is nailed down: the track the overview was measured from paints all
//! four of its hot cues `#3CEB50` and stores index 21 for each. This prints how
//! much of the library that one index covers, and shows the memory cue sitting
//! at the same millisecond as each hot cue — which is what the small red head
//! beside every badge in the capture turned out to be.
//!
//! `cargo run -p rbl-db --example hotcue_palette`
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
    for (label, sql) in [
        ("hot cues by ColorTableIndex",
         "SELECT ColorTableIndex, COUNT(*) FROM djmdCue WHERE rb_local_deleted = 0 AND Kind <> 0 \
          GROUP BY ColorTableIndex ORDER BY COUNT(*) DESC LIMIT 12"),
        ("Kind, ColorTableIndex, Color and position of the measured track's cues",
         "SELECT c.Kind, c.ColorTableIndex, c.Color, c.InMsec FROM djmdCue c \
          JOIN djmdContent t ON t.ID = c.ContentID \
          WHERE c.rb_local_deleted = 0 AND t.Title LIKE 'Take Me Home (ft. Bonn)%' ORDER BY c.InMsec"),
    ] {
        println!("\n{label}");
        let mut stmt = match conn.prepare(sql) {
            Ok(stmt) => stmt,
            Err(e) => {
                println!("  cannot read: {e}");
                continue;
            }
        };
        let cols = stmt.column_count();
        let mut rows = match stmt.query([]) {
            Ok(rows) => rows,
            Err(e) => {
                println!("  cannot read: {e}");
                continue;
            }
        };
        while let Ok(Some(row)) = rows.next() {
            let cells: Vec<String> = (0..cols)
                .map(|i| row.get_ref(i).map_or_else(|_| "?".to_owned(), |v| format!("{v:?}")))
                .collect();
            println!("  {}", cells.join("  "));
        }
    }
}
