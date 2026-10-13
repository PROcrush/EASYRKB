//! Differential recorder for rekordbox's database.
//!
//! Several things about `master.db` are documented nowhere: what the `Analysed`
//! bitfield means, how `rb_data_status` is used, how `BeatLoopSize` is encoded,
//! how hot-cue palette indices map to colours. Guessing them and writing the
//! guess into someone's library is how libraries get corrupted.
//!
//! Instead: snapshot the database, let rekordbox itself perform one action,
//! snapshot again, and diff. The diff *is* the specification.
//!
//! Everything here is read-only with respect to `master.db`.

use std::collections::BTreeMap;
use std::path::Path;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum DiffError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, DiffError>;

/// Tables worth recording. `djmd*` is the library proper; the others carry
/// derived state whose ownership is not yet understood.
pub const TRACKED_PREFIXES: &[&str] = &["djmd", "content", "agentRegistry", "cloud", "hotCue"];

/// One row, as a map of column name to rendered value.
pub type Row = BTreeMap<String, String>;

/// Tables at or below this many rows are stored in full; larger ones keep a
/// per-row digest instead.
///
/// A full snapshot of the user's library is 2.36M rows and came to 2.8 GB of
/// JSON, which is unusable for the eight recordings the plan calls for. The
/// tables that actually change during a single rekordbox action — playlists,
/// the agent registry, properties — are small and are still captured in full;
/// the bulk tables keep digests, which is enough to say *which* rows moved.
pub const FULL_ROW_LIMIT: usize = 5_000;

/// A table's rows keyed by primary key (or by row index when there is no `ID`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TableSnapshot {
    /// Present when the table is at or below [`FULL_ROW_LIMIT`].
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rows: BTreeMap<String, Row>,
    /// Always present: key -> digest of the whole row.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub digests: BTreeMap<String, u64>,
    pub row_count: usize,
    /// False when only digests were kept.
    pub full: bool,
}

impl TableSnapshot {
    fn keys(&self) -> Vec<&String> {
        if self.full { self.rows.keys().collect() } else { self.digests.keys().collect() }
    }
    fn digest_of(&self, key: &str) -> Option<u64> {
        if self.full { self.rows.get(key).map(digest_row) } else { self.digests.get(key).copied() }
    }
}

/// FNV-1a over the row's fields, order-independent per key.
fn digest_row(row: &Row) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for (k, v) in row {
        for byte in k.as_bytes().iter().chain(b"=").chain(v.as_bytes()).chain(b";") {
            h ^= u64::from(*byte);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Snapshot {
    /// When this snapshot was taken, in the same format rekordbox writes.
    pub taken_at: String,
    pub tables: BTreeMap<String, TableSnapshot>,
}

/// What changed between two snapshots.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Diff {
    pub added: BTreeMap<String, Vec<Row>>,
    /// Table -> key -> (column -> [before, after]).
    pub changed: BTreeMap<String, BTreeMap<String, BTreeMap<String, [String; 2]>>>,
    pub removed: BTreeMap<String, Vec<String>>,
    /// Rows that changed in a digest-only table: we know the key moved but not
    /// which column. `rbl-difftool inspect <table> <key>` reads the row.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub changed_keys_only: BTreeMap<String, Vec<String>>,
    /// Keys added or removed in a digest-only table.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub added_keys_only: BTreeMap<String, Vec<String>>,
}

impl Diff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.changed.is_empty()
            && self.removed.is_empty()
            && self.changed_keys_only.is_empty()
            && self.added_keys_only.is_empty()
    }

    /// One-line-per-change rendering, which is what makes a recording readable
    /// as documentation.
    pub fn summarise(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        for (table, rows) in &self.added {
            let _ = writeln!(out, "+ {table}: {} row(s) added", rows.len());
            for row in rows.iter().take(3) {
                let _ = writeln!(out, "    {}", render(row));
            }
        }
        for (table, keys) in &self.changed {
            let _ = writeln!(out, "~ {table}: {} row(s) changed", keys.len());
            for (key, columns) in keys.iter().take(5) {
                let fields: Vec<String> = columns
                    .iter()
                    .map(|(c, [before, after])| format!("{c}: {before:?} -> {after:?}"))
                    .collect();
                let _ = writeln!(out, "    [{key}] {}", fields.join(", "));
            }
        }
        for (table, keys) in &self.added_keys_only {
            let _ = writeln!(out, 
                "+ {table}: {} row(s) added (digest-only table; inspect: {})",
                keys.len(),
                keys.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
            );
        }
        for (table, keys) in &self.changed_keys_only {
            let _ = writeln!(out, 
                "~ {table}: {} row(s) changed (digest-only table; inspect: {})",
                keys.len(),
                keys.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
            );
        }
        for (table, keys) in &self.removed {
            let _ = writeln!(out, "- {table}: {} row(s) removed ({})", keys.len(),
                keys.iter().take(5).cloned().collect::<Vec<_>>().join(", "));
        }
        if out.is_empty() {
            out.push_str("(no changes)\n");
        }
        out
    }
}

fn render(row: &Row) -> String {
    row.iter()
        .filter(|(_, v)| !v.is_empty() && v.as_str() != "NULL")
        .take(8)
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Tables to snapshot, filtered to the tracked prefixes.
fn tracked_tables(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name",
    )?;
    let names = stmt.query_map([], |r| r.get::<_, String>(0))?;
    Ok(names
        .filter_map(std::result::Result::ok)
        .filter(|n| TRACKED_PREFIXES.iter().any(|p| n.starts_with(p)))
        .collect())
}

fn value_to_string(value: &rusqlite::types::ValueRef<'_>) -> String {
    use rusqlite::types::ValueRef;
    match value {
        ValueRef::Null => "NULL".to_owned(),
        ValueRef::Integer(v) => v.to_string(),
        ValueRef::Real(v) => v.to_string(),
        ValueRef::Text(t) => String::from_utf8_lossy(t).into_owned(),
        // Blobs are summarised: their content is not what a recording is for,
        // but a change in one still shows up as a different digest.
        ValueRef::Blob(b) => format!("<blob {} bytes, xor {:02x}>", b.len(),
            b.iter().fold(0_u8, |a, x| a ^ x)),
    }
}

/// Reads every tracked table. Read-only.
pub fn snapshot(conn: &Connection) -> Result<Snapshot> {
    let mut tables = BTreeMap::new();
    for table in tracked_tables(conn)? {
        // Identifier comes from sqlite_master, not from user input.
        let mut stmt = match conn.prepare(&format!("SELECT * FROM {table}")) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(table, error = %e, "skipping table");
                continue;
            }
        };
        let columns: Vec<String> = stmt.column_names().iter().map(|s| (*s).to_owned()).collect();
        let has_id = columns.iter().any(|c| c == "ID");

        // How many rows are there? Decides full-row versus digest storage.
        let row_count: usize = conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get::<_, i64>(0))
            .map_or(0, |n| usize::try_from(n).unwrap_or(0));
        let full = row_count <= FULL_ROW_LIMIT;

        let mut snapshot = TableSnapshot { row_count, full, ..TableSnapshot::default() };
        let mut rows = stmt.query([])?;
        let mut index = 0_u64;
        while let Some(row) = rows.next()? {
            let mut fields = Row::new();
            for (i, name) in columns.iter().enumerate() {
                let value = row.get_ref(i).map(|v| value_to_string(&v)).unwrap_or_default();
                fields.insert(name.clone(), value);
            }
            let key = if has_id {
                fields.get("ID").cloned().unwrap_or_else(|| index.to_string())
            } else {
                index.to_string()
            };
            if full {
                snapshot.rows.insert(key, fields);
            } else {
                snapshot.digests.insert(key, digest_row(&fields));
            }
            index += 1;
        }
        tables.insert(table, snapshot);
    }

    Ok(Snapshot { taken_at: now_utc(), tables })
}

/// Compares two snapshots.
pub fn diff(before: &Snapshot, after: &Snapshot) -> Diff {
    let mut out = Diff::default();

    for (table, after_table) in &after.tables {
        let empty = TableSnapshot::default();
        let before_table = before.tables.get(table).unwrap_or(&empty);
        let both_full = after_table.full && before_table.full;

        if both_full {
            for (key, after_row) in &after_table.rows {
                match before_table.rows.get(key) {
                    None => out.added.entry(table.clone()).or_default().push(after_row.clone()),
                    Some(before_row) => {
                        let mut changed = BTreeMap::new();
                        for (column, after_value) in after_row {
                            let before_value = before_row.get(column).map_or("", String::as_str);
                            if before_value != after_value {
                                changed.insert(
                                    column.clone(),
                                    [before_value.to_owned(), after_value.clone()],
                                );
                            }
                        }
                        if !changed.is_empty() {
                            out.changed
                                .entry(table.clone())
                                .or_default()
                                .insert(key.clone(), changed);
                        }
                    }
                }
            }
        } else {
            // At least one side kept digests only: we can say which rows moved,
            // not which column. `inspect` reads the row from the live database.
            for key in after_table.keys() {
                match before_table.digest_of(key) {
                    None => out.added_keys_only.entry(table.clone()).or_default().push(key.clone()),
                    Some(before_digest) => {
                        if after_table.digest_of(key) != Some(before_digest) {
                            out.changed_keys_only
                                .entry(table.clone())
                                .or_default()
                                .push(key.clone());
                        }
                    }
                }
            }
        }

        for key in before_table.keys() {
            let gone = if after_table.full {
                !after_table.rows.contains_key(key)
            } else {
                !after_table.digests.contains_key(key)
            };
            if gone {
                out.removed.entry(table.clone()).or_default().push(key.clone());
            }
        }
    }

    out
}

/// Snapshots are gzipped: the JSON is highly repetitive and compresses about
/// twenty-fold, which is the difference between a usable recording and a file
/// too large to keep beside the code.
pub fn write_snapshot(path: &Path, snapshot: &Snapshot) -> Result<()> {
    use std::io::Write as _;
    let json = serde_json::to_vec(snapshot)?;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&json)?;
    std::fs::write(path, encoder.finish()?)?;
    Ok(())
}

pub fn read_snapshot(path: &Path) -> Result<Snapshot> {
    use std::io::Read as _;
    let bytes = std::fs::read(path)?;
    // Accept plain JSON too, so a hand-edited snapshot still loads.
    if bytes.first() == Some(&0x1f) && bytes.get(1) == Some(&0x8b) {
        let mut decoder = flate2::read::GzDecoder::new(bytes.as_slice());
        let mut json = Vec::new();
        decoder.read_to_end(&mut json)?;
        Ok(serde_json::from_slice(&json)?)
    } else {
        Ok(serde_json::from_slice(&bytes)?)
    }
}

/// rekordbox's timestamp format, so recordings sort next to database values.
fn now_utc() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let millis = now.subsec_millis();
    let days = secs / 86_400;
    let time = secs % 86_400;
    // Civil-from-days, valid for any date after 1970.
    let (y, m, d) = civil_from_days(i64::try_from(days).unwrap_or(0));
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}.{millis:03} +00:00",
        time / 3600,
        (time % 3600) / 60,
        time % 60
    )
}

/// Howard Hinnant's days-to-civil algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    // Both are bounded to 1..=31 and 1..=12 by the algorithm, so the
    // conversion cannot fail; fall back rather than cast blindly.
    let d = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let m = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn row(pairs: &[(&str, &str)]) -> Row {
        pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
    }

    fn snap(table: &str, rows: &[(&str, Row)]) -> Snapshot {
        let mut t = TableSnapshot { full: true, ..TableSnapshot::default() };
        for (key, r) in rows {
            t.rows.insert((*key).to_owned(), r.clone());
        }
        t.row_count = t.rows.len();
        Snapshot {
            taken_at: "2026-09-07 00:00:00.000 +00:00".to_owned(),
            tables: [(table.to_owned(), t)].into_iter().collect(),
        }
    }

    #[test]
    fn detects_an_added_row() {
        let before = snap("djmdPlaylist", &[]);
        let after = snap("djmdPlaylist", &[("7", row(&[("ID", "7"), ("Name", "Set")]))]);
        let d = diff(&before, &after);
        assert_eq!(d.added.get("djmdPlaylist").unwrap().len(), 1);
        assert!(d.changed.is_empty() && d.removed.is_empty());
    }

    #[test]
    fn detects_a_changed_field_and_records_both_values() {
        let before = snap("djmdContent", &[("1", row(&[("ID", "1"), ("Analysed", "0")]))]);
        let after = snap("djmdContent", &[("1", row(&[("ID", "1"), ("Analysed", "105")]))]);
        let d = diff(&before, &after);
        let change = &d.changed["djmdContent"]["1"]["Analysed"];
        assert_eq!(change, &["0".to_owned(), "105".to_owned()]);
    }

    #[test]
    fn detects_a_removed_row() {
        let before = snap("djmdPlaylist", &[("7", row(&[("ID", "7")]))]);
        let after = snap("djmdPlaylist", &[]);
        assert_eq!(diff(&before, &after).removed.get("djmdPlaylist").unwrap().len(), 1);
    }

    #[test]
    fn an_unchanged_database_produces_an_empty_diff() {
        let s = snap("djmdContent", &[("1", row(&[("ID", "1"), ("Title", "x")]))]);
        let d = diff(&s, &s);
        assert!(d.is_empty());
        assert_eq!(d.summarise(), "(no changes)\n");
    }

    #[test]
    fn summary_names_the_columns_that_moved() {
        let before = snap("djmdContent", &[("1", row(&[("ID", "1"), ("rb_local_usn", "10")]))]);
        let after = snap("djmdContent", &[("1", row(&[("ID", "1"), ("rb_local_usn", "11")]))]);
        let text = diff(&before, &after).summarise();
        assert!(text.contains("rb_local_usn"), "{text}");
        assert!(text.contains("\"10\" -> \"11\""), "{text}");
    }

    #[test]
    fn snapshots_round_trip_through_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.json");
        let s = snap("djmdContent", &[("1", row(&[("ID", "1")]))]);
        write_snapshot(&path, &s).unwrap();
        let back = read_snapshot(&path).unwrap();
        assert_eq!(back.tables["djmdContent"].rows.len(), 1);
    }

    #[test]
    fn snapshots_a_real_sqlite_database() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.db");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE djmdContent (ID TEXT PRIMARY KEY, Title TEXT, Analysed INTEGER);
             INSERT INTO djmdContent VALUES ('1', 'A', 0), ('2', 'B', NULL);
             CREATE TABLE unrelated (x INTEGER);
             INSERT INTO unrelated VALUES (1);",
        )
        .unwrap();

        let before = snapshot(&conn).unwrap();
        assert!(before.tables.contains_key("djmdContent"));
        assert!(!before.tables.contains_key("unrelated"), "untracked tables are skipped");
        assert_eq!(before.tables["djmdContent"].rows["2"]["Analysed"], "NULL");

        conn.execute("UPDATE djmdContent SET Analysed = 105 WHERE ID = '1'", []).unwrap();
        conn.execute("INSERT INTO djmdContent VALUES ('3', 'C', 1)", []).unwrap();
        let after = snapshot(&conn).unwrap();

        let d = diff(&before, &after);
        assert_eq!(d.added["djmdContent"].len(), 1);
        assert_eq!(d.changed["djmdContent"]["1"]["Analysed"], ["0".to_owned(), "105".to_owned()]);
    }

    #[test]
    fn timestamps_match_rekordboxs_format() {
        let t = now_utc();
        assert_eq!(t.len(), "2026-09-07 00:00:00.000 +00:00".len(), "{t}");
        assert!(t.ends_with(" +00:00"), "{t}");
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_000), (2022, 1, 8));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::pedantic)]
mod large_table_tests {
    use super::*;

    /// Builds a database whose table exceeds the full-row limit.
    fn big_db(dir: &Path, rows: usize) -> Connection {
        let conn = Connection::open(dir.join("big.db")).unwrap();
        conn.execute_batch("CREATE TABLE djmdCue (ID TEXT PRIMARY KEY, InMsec INTEGER)").unwrap();
        let tx = conn.unchecked_transaction().unwrap();
        for i in 0..rows {
            tx.execute("INSERT INTO djmdCue VALUES (?1, ?2)", rusqlite::params![i.to_string(), i as i64])
                .unwrap();
        }
        tx.commit().unwrap();
        conn
    }

    #[test]
    fn a_large_table_keeps_digests_not_rows() {
        let dir = tempfile::tempdir().unwrap();
        let conn = big_db(dir.path(), FULL_ROW_LIMIT + 10);
        let snap = snapshot(&conn).unwrap();
        let t = &snap.tables["djmdCue"];
        assert!(!t.full, "should have fallen back to digests");
        assert!(t.rows.is_empty());
        assert_eq!(t.digests.len(), FULL_ROW_LIMIT + 10);
        assert_eq!(t.row_count, FULL_ROW_LIMIT + 10);
    }

    #[test]
    fn a_change_in_a_large_table_is_still_detected() {
        let dir = tempfile::tempdir().unwrap();
        let conn = big_db(dir.path(), FULL_ROW_LIMIT + 10);
        let before = snapshot(&conn).unwrap();
        conn.execute("UPDATE djmdCue SET InMsec = 999 WHERE ID = '42'", []).unwrap();
        conn.execute("INSERT INTO djmdCue VALUES ('999999', 1)", []).unwrap();
        conn.execute("DELETE FROM djmdCue WHERE ID = '7'", []).unwrap();
        let after = snapshot(&conn).unwrap();

        let d = diff(&before, &after);
        assert_eq!(d.changed_keys_only["djmdCue"], vec!["42".to_owned()]);
        assert_eq!(d.added_keys_only["djmdCue"], vec!["999999".to_owned()]);
        assert_eq!(d.removed["djmdCue"], vec!["7".to_owned()]);
        assert!(d.summarise().contains("digest-only"));
    }

    #[test]
    fn a_gzipped_snapshot_is_much_smaller_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let conn = big_db(dir.path(), FULL_ROW_LIMIT + 10);
        let snap = snapshot(&conn).unwrap();
        let path = dir.path().join("s.json.gz");
        write_snapshot(&path, &snap).unwrap();

        let raw = serde_json::to_vec(&snap).unwrap().len();
        let compressed = std::fs::metadata(&path).unwrap().len() as usize;
        // Digests are high-entropy decimals, so this table compresses only ~2x;
        // the large gains are on the text-heavy tables in a real library.
        assert!(compressed < raw, "expected compression: {compressed} vs {raw}");

        let back = read_snapshot(&path).unwrap();
        assert_eq!(back.tables["djmdCue"].digests.len(), snap.tables["djmdCue"].digests.len());
    }

    #[test]
    fn plain_json_snapshots_still_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plain.json");
        let s = Snapshot::default();
        std::fs::write(&path, serde_json::to_vec(&s).unwrap()).unwrap();
        assert!(read_snapshot(&path).is_ok());
    }
}
