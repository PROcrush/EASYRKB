//! What `djmdCue` holds. READ-ONLY.
//!
//! `cargo run -p rbl-db --example cues`
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
        ("cues by Kind (0 = memory, hot cues above)",
         "SELECT Kind, COUNT(*) FROM djmdCue WHERE rb_local_deleted = 0 GROUP BY Kind ORDER BY Kind"),
        ("how many carry a Color, and which",
         "SELECT Color, COUNT(*) FROM djmdCue WHERE rb_local_deleted = 0 GROUP BY Color ORDER BY COUNT(*) DESC LIMIT 8"),
        ("ColorTableIndex values",
         "SELECT ColorTableIndex, COUNT(*) FROM djmdCue WHERE rb_local_deleted = 0 GROUP BY ColorTableIndex ORDER BY COUNT(*) DESC LIMIT 8"),
        ("loops: how many have an OutMsec",
         "SELECT (OutMsec IS NOT NULL AND OutMsec > 0) AS is_loop, COUNT(*) FROM djmdCue WHERE rb_local_deleted = 0 GROUP BY 1"),
        ("tracks with any cue",
         "SELECT COUNT(DISTINCT ContentID) FROM djmdCue WHERE rb_local_deleted = 0"),
        ("most cues on one track",
         "SELECT COUNT(*) FROM djmdCue WHERE rb_local_deleted = 0 GROUP BY ContentID ORDER BY COUNT(*) DESC LIMIT 1"),
        ("ColorTableIndex by Kind — is it a palette, or a per-slot default?",
         "SELECT Kind, ColorTableIndex, COUNT(*) FROM djmdCue WHERE rb_local_deleted = 0 \
          GROUP BY Kind, ColorTableIndex ORDER BY Kind, COUNT(*) DESC"),
        ("distinct ColorTableIndex per Kind",
         "SELECT Kind, COUNT(DISTINCT ColorTableIndex) FROM djmdCue WHERE rb_local_deleted = 0 GROUP BY Kind"),
        ("BeatLoopSize — set on what?",
         "SELECT (OutMsec IS NOT NULL AND OutMsec > 0) AS is_loop, BeatLoopSize, COUNT(*) \
          FROM djmdCue WHERE rb_local_deleted = 0 GROUP BY 1,2 ORDER BY 1, COUNT(*) DESC LIMIT 10"),
        ("what else a plain hot cue carries",
         "SELECT InFrame, InMpegFrame, InMpegAbs, ActiveLoop, CueMicrosec, COUNT(*) \
          FROM djmdCue WHERE rb_local_deleted = 0 AND Kind = 1 GROUP BY 1,2,3,4,5 \
          ORDER BY COUNT(*) DESC LIMIT 4"),
        ("Color by Kind",
         "SELECT Kind, Color, COUNT(*) FROM djmdCue WHERE rb_local_deleted = 0 \
          GROUP BY 1,2 ORDER BY 1, COUNT(*) DESC LIMIT 8"),
        ("loops: does BeatLoopSize>>16 equal the loop's length in beats?",
         "SELECT c.BeatLoopSize, c.OutMsec - c.InMsec AS ms, t.BPM, \
                 ROUND((c.OutMsec - c.InMsec) * (t.BPM / 100.0) / 60000.0, 2) AS beats \
          FROM djmdCue c JOIN djmdContent t ON t.ID = c.ContentID \
          WHERE c.rb_local_deleted = 0 AND c.OutMsec > 0 AND t.BPM > 0 \
          AND c.BeatLoopSize > 0 ORDER BY c.BeatLoopSize DESC LIMIT 14"),
        ("a sample track's cues",
         "SELECT Kind, InMsec, OutMsec, Color, ColorTableIndex, Comment FROM djmdCue \
          WHERE rb_local_deleted = 0 AND ContentID = (
            SELECT ContentID FROM djmdCue WHERE rb_local_deleted = 0
            GROUP BY ContentID ORDER BY COUNT(*) DESC LIMIT 1) ORDER BY Kind, InMsec"),
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
                    Ok(rusqlite::types::ValueRef::Null) => "NULL".to_owned(),
                    Ok(rusqlite::types::ValueRef::Integer(v)) => v.to_string(),
                    Ok(rusqlite::types::ValueRef::Text(v)) => {
                        String::from_utf8_lossy(v).into_owned()
                    }
                    other => format!("{other:?}"),
                })
                .collect();
            println!("  {}", cells.join(" | "));
        }
        println!();
    }
}
