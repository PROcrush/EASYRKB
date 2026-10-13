//! What the columns behind the information panel's Summary and Info tabs hold.
//! READ-ONLY.
//!
//! `cargo run -p rbl-db --example track_details`
//!
//! Counts the population of each column the panel shows rather than reading
//! one row, because one row cannot say whether a value is a convention or a
//! coincidence: `FileType` against the file's extension, what `HotCueAutoLoad`
//! and `DeliveryControl` are spelled as, whether `OrgArtistID` / `ComposerID`
//! / `RemixerID` point into `djmdArtist`, whether the album artist lives on
//! the album, and how `DateCreated` is written.
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use rusqlite::Connection;

fn count(conn: &Connection, sql: &str) {
    // perf-ok: a read-only probe run by hand, not shipped code.
    let mut stmt = conn.prepare(sql).expect("prepare");
    let names: Vec<String> = stmt.column_names().iter().map(|s| (*s).to_owned()).collect();
    let rows = stmt
        .query_map([], |r| {
            let mut out = Vec::new();
            for i in 0..names.len() {
                let v = r.get::<_, rusqlite::types::Value>(i).unwrap_or(rusqlite::types::Value::Null);
                out.push(match v {
                    rusqlite::types::Value::Null => "NULL".to_owned(),
                    rusqlite::types::Value::Integer(i) => i.to_string(),
                    rusqlite::types::Value::Real(f) => f.to_string(),
                    rusqlite::types::Value::Text(t) => format!("{t:?}"),
                    rusqlite::types::Value::Blob(b) => format!("<{} bytes>", b.len()),
                });
            }
            Ok(out)
        })
        .expect("query"); // perf-ok: a read-only probe run by hand.
    // perf-ok: printing is the point of a probe.
    println!("-- {}", sql.split_whitespace().collect::<Vec<_>>().join(" "));
    for row in rows.filter_map(Result::ok) {
        println!("   {}", row.join(" | ")); // perf-ok: probe output.
    }
    println!(); // perf-ok: probe output.
}

fn main() {
    let db = match rbl_db::Library::open_installed_read_only() {
        Ok(db) => db,
        Err(e) => {
            println!("cannot open library: {e}"); // perf-ok: probe output.
            return;
        }
    };
    let conn = db.connection();
    let share = db.location().share_root.clone();
    println!("share root: {}\n", share.display()); // perf-ok: probe output.

    // FileType against the extension it should follow from.
    count(
        conn,
        "SELECT FileType, lower(substr(FileNameL, -4)) AS ext, COUNT(*) AS n
         FROM djmdContent WHERE rb_local_deleted = 0
         GROUP BY FileType, ext ORDER BY n DESC LIMIT 30",
    );
    // The two checkboxes.
    count(
        conn,
        "SELECT HotCueAutoLoad, COUNT(*) FROM djmdContent WHERE rb_local_deleted = 0
         GROUP BY HotCueAutoLoad",
    );
    count(
        conn,
        "SELECT DeliveryControl, COUNT(*) FROM djmdContent WHERE rb_local_deleted = 0
         GROUP BY DeliveryControl",
    );
    count(
        conn,
        "SELECT COUNT(*), COUNT(DeliveryComment),
                SUM(CASE WHEN DeliveryComment != '' THEN 1 ELSE 0 END)
         FROM djmdContent WHERE rb_local_deleted = 0",
    );
    count(
        conn,
        "SELECT DeliveryComment FROM djmdContent WHERE rb_local_deleted = 0
         AND DeliveryComment IS NOT NULL AND DeliveryComment != '' LIMIT 5",
    );
    // Mix name candidate.
    count(
        conn,
        "SELECT COUNT(*), SUM(CASE WHEN Subtitle IS NOT NULL AND Subtitle != '' THEN 1 ELSE 0 END)
         FROM djmdContent WHERE rb_local_deleted = 0",
    );
    count(
        conn,
        "SELECT Title, Subtitle FROM djmdContent WHERE rb_local_deleted = 0
         AND Subtitle IS NOT NULL AND Subtitle != '' LIMIT 8",
    );
    // Lyricist is a plain column.
    count(
        conn,
        "SELECT COUNT(*), SUM(CASE WHEN Lyricist IS NOT NULL AND Lyricist != '' THEN 1 ELSE 0 END)
         FROM djmdContent WHERE rb_local_deleted = 0",
    );
    // The three artist-shaped foreign keys.
    for column in ["OrgArtistID", "ComposerID", "RemixerID", "ArtistID"] {
        count(
            conn,
            &format!(
                "SELECT COUNT(*) AS set_,
                        SUM(CASE WHEN a.ID IS NOT NULL THEN 1 ELSE 0 END) AS resolves
                 FROM djmdContent c LEFT JOIN djmdArtist a ON a.ID = c.{column}
                 WHERE c.rb_local_deleted = 0 AND c.{column} IS NOT NULL AND c.{column} != ''"
            ),
        );
    }
    // Album artist lives on the album.
    count(
        conn,
        "SELECT COUNT(*) AS albums,
                SUM(CASE WHEN AlbumArtistID IS NOT NULL AND AlbumArtistID != '' THEN 1 ELSE 0 END) AS with_artist,
                SUM(CASE WHEN a.ID IS NOT NULL THEN 1 ELSE 0 END) AS resolves
         FROM djmdAlbum al LEFT JOIN djmdArtist a ON a.ID = al.AlbumArtistID
         WHERE al.rb_local_deleted = 0",
    );
    // Dates and sizes.
    count(
        conn,
        "SELECT DateCreated, StockDate, ReleaseDate, ReleaseYear, created_at
         FROM djmdContent WHERE rb_local_deleted = 0 ORDER BY created_at DESC LIMIT 5",
    );
    count(
        conn,
        "SELECT length(DateCreated) AS len, COUNT(*) FROM djmdContent WHERE rb_local_deleted = 0
         GROUP BY len",
    );
    count(
        conn,
        "SELECT BitDepth, COUNT(*) FROM djmdContent WHERE rb_local_deleted = 0 GROUP BY BitDepth",
    );
    count(
        conn,
        "SELECT SampleRate, COUNT(*) FROM djmdContent WHERE rb_local_deleted = 0
         GROUP BY SampleRate ORDER BY 2 DESC LIMIT 8",
    );
    count(
        conn,
        "SELECT BitRate, COUNT(*) FROM djmdContent WHERE rb_local_deleted = 0
         GROUP BY BitRate ORDER BY 2 DESC LIMIT 8",
    );
    count(
        conn,
        "SELECT ReleaseYear, COUNT(*) FROM djmdContent WHERE rb_local_deleted = 0
         GROUP BY ReleaseYear ORDER BY 2 DESC LIMIT 6",
    );
    count(
        conn,
        "SELECT DiscNo, COUNT(*) FROM djmdContent WHERE rb_local_deleted = 0
         GROUP BY DiscNo ORDER BY 2 DESC LIMIT 6",
    );
    count(
        conn,
        "SELECT TrackNo, COUNT(*) FROM djmdContent WHERE rb_local_deleted = 0
         GROUP BY TrackNo ORDER BY 2 DESC LIMIT 6",
    );
    // Keys and colours the dropdowns offer.
    count(conn, "SELECT ID, ScaleName, Seq FROM djmdKey WHERE rb_local_deleted = 0 ORDER BY Seq");
    count(conn, "SELECT ID, ColorCode, SortKey, Commnt FROM djmdColor ORDER BY SortKey");
    count(
        conn,
        "SELECT ColorID, COUNT(*) FROM djmdContent WHERE rb_local_deleted = 0 GROUP BY ColorID",
    );
    // Artwork.
    count(
        conn,
        "SELECT ImagePath FROM djmdContent WHERE rb_local_deleted = 0
         AND ImagePath IS NOT NULL AND ImagePath != '' ORDER BY updated_at DESC LIMIT 5",
    );
    count(
        conn,
        "SELECT substr(ImagePath, 1, 40) AS prefix, COUNT(*) FROM djmdContent
         WHERE rb_local_deleted = 0 AND ImagePath IS NOT NULL AND ImagePath != ''
         GROUP BY prefix ORDER BY 2 DESC LIMIT 8",
    );
    count(
        conn,
        "SELECT ID, Title, FileType, FileSize, BitRate, SampleRate, BitDepth, DateCreated,
                DJPlayCount, ReleaseYear, TrackNo, DiscNo, HotCueAutoLoad, DeliveryControl
         FROM djmdContent WHERE rb_local_deleted = 0 AND Title LIKE 'Age Of Love (Dominant%'",
    );
    conventions(conn);
}

/// Whether rekordbox maintains `SearchStr` alongside the columns the panel
/// edits, how an absent reference is spelled (NULL or empty), and whether
/// `Subtitle` reads as the mix name across the whole population.
fn conventions(conn: &Connection) {
    count(
        conn,
        "SELECT COUNT(*), COUNT(SearchStr), SUM(CASE WHEN SearchStr != '' THEN 1 ELSE 0 END)
         FROM djmdContent WHERE rb_local_deleted = 0",
    );
    count(
        conn,
        "SELECT Title, SearchStr FROM djmdContent WHERE rb_local_deleted = 0
         AND SearchStr IS NOT NULL AND SearchStr != '' LIMIT 4",
    );
    count(
        conn,
        "SELECT COUNT(*), COUNT(SearchStr), SUM(CASE WHEN SearchStr != '' THEN 1 ELSE 0 END)
         FROM djmdArtist WHERE rb_local_deleted = 0",
    );
    count(
        conn,
        "SELECT SUM(CASE WHEN ArtistID IS NULL THEN 1 ELSE 0 END) AS null_,
                SUM(CASE WHEN ArtistID = '' THEN 1 ELSE 0 END) AS empty_,
                SUM(CASE WHEN AlbumID IS NULL THEN 1 ELSE 0 END) AS album_null,
                SUM(CASE WHEN AlbumID = '' THEN 1 ELSE 0 END) AS album_empty,
                SUM(CASE WHEN KeyID IS NULL THEN 1 ELSE 0 END) AS key_null,
                SUM(CASE WHEN KeyID = '' THEN 1 ELSE 0 END) AS key_empty,
                SUM(CASE WHEN ReleaseDate IS NULL THEN 1 ELSE 0 END) AS rd_null,
                SUM(CASE WHEN ReleaseDate = '' THEN 1 ELSE 0 END) AS rd_empty,
                SUM(CASE WHEN Subtitle IS NULL THEN 1 ELSE 0 END) AS sub_null,
                SUM(CASE WHEN Lyricist IS NULL THEN 1 ELSE 0 END) AS lyr_null,
                SUM(CASE WHEN ImagePath IS NULL THEN 1 ELSE 0 END) AS img_null,
                SUM(CASE WHEN ImagePath = '' THEN 1 ELSE 0 END) AS img_empty
         FROM djmdContent WHERE rb_local_deleted = 0",
    );
    count(conn, "SELECT COUNT(*) FROM djmdGenre WHERE rb_local_deleted = 0");
    count(conn, "SELECT COUNT(*) FROM djmdKey WHERE rb_local_deleted = 0");
    count(
        conn,
        "SELECT ScaleName, COUNT(*) FROM djmdKey WHERE rb_local_deleted = 0
         GROUP BY ScaleName HAVING COUNT(*) > 1",
    );
    count(
        conn,
        "SELECT Title, Subtitle FROM djmdContent WHERE rb_local_deleted = 0
         AND Subtitle IS NOT NULL AND Subtitle != ''
         AND lower(Title) NOT LIKE '%' || lower(Subtitle) || '%' LIMIT 12",
    );
    count(
        conn,
        "SELECT COUNT(*) FROM djmdContent WHERE rb_local_deleted = 0
         AND Subtitle IS NOT NULL AND Subtitle != ''
         AND lower(Title) NOT LIKE '%' || lower(Subtitle) || '%'",
    );
}
