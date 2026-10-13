//! `summary.json`: what a backup holds, for a person to recognise it by
//! before restoring — how many tracks, playlists and cues, and the size of
//! each kind of data.
//!
//! RBXport writes one into every backup. For a backup made before that,
//! [`from_archive`] works the same figures out by reading the archive.
use crate::{
    archive::{self, Entry},
    manifest::Manifest,
    refused,
    sizes::{self, BackupSizes, Measurement},
    Result,
};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path, sync::Mutex};

pub const NAME: &str = "summary.json";
pub const VERSION: u32 = 1;
const MAX_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub version: u32,
    /// Unix milliseconds.
    pub created_at: u64,
    /// The RBXport that wrote the backup. `None` when worked out afterwards.
    #[serde(default)]
    pub app_version: Option<String>,
    pub sizes: BackupSizes,
    pub counts: Counts,
}

/// Library contents. Every figure counts live rows only, not ones rekordbox
/// has marked deleted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Counts {
    pub tracks: u64,
    /// Tracks with an analysis file recorded.
    pub analyzed_tracks: u64,
    pub playlists: u64,
    pub smart_playlists: u64,
    pub playlist_folders: u64,
    /// Hot cues and hot loops, A to P.
    pub hot_cues: u64,
    /// Memory cues and memory loops.
    pub memory_cues: u64,
    pub my_tags: u64,
    pub history_sessions: u64,
    pub analysis_files: u64,
    pub artwork_files: u64,
}

fn has_table(conn: &rusqlite::Connection, name: &str) -> rusqlite::Result<bool> {
    conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1", [name], |r| r.get::<_, i64>(0))
        .map(|count| count > 0)
}

fn number(conn: &rusqlite::Connection, table: &str, sql: &str) -> rusqlite::Result<u64> {
    if !has_table(conn, table)? {
        return Ok(0);
    }
    let value: Option<i64> = conn.query_row(sql, [], |r| r.get(0))?;
    Ok(value.and_then(|value| u64::try_from(value).ok()).unwrap_or(0))
}

/// Counts a library database holds. The file counts are left at zero: they
/// come from the analysis and artwork trees.
pub fn count(conn: &rusqlite::Connection) -> rusqlite::Result<Counts> {
    let live_tracks = "SELECT ID FROM djmdContent WHERE rb_local_deleted = 0";
    let playlists = |attribute: i64| {
        number(conn, "djmdPlaylist", &format!("SELECT COUNT(*) FROM djmdPlaylist WHERE rb_local_deleted = 0 AND Attribute = {attribute}"))
    };
    Ok(Counts {
        tracks: number(conn, "djmdContent", "SELECT COUNT(*) FROM djmdContent WHERE rb_local_deleted = 0")?,
        analyzed_tracks: number(conn, "djmdContent", "SELECT COUNT(*) FROM djmdContent WHERE rb_local_deleted = 0 AND COALESCE(AnalysisDataPath, '') <> ''")?,
        playlists: playlists(rbl_db::write::ATTRIBUTE_PLAYLIST)?,
        smart_playlists: playlists(rbl_db::write::ATTRIBUTE_SMART)?,
        playlist_folders: playlists(rbl_db::write::ATTRIBUTE_FOLDER)?,
        // Kind 0 is a memory cue; 1 to 3 and 5 to 17 are hot cues A to P.
        hot_cues: number(conn, "djmdCue", &format!("SELECT COUNT(*) FROM djmdCue WHERE rb_local_deleted = 0 AND Kind BETWEEN 1 AND 17 AND Kind <> 4 AND ContentID IN ({live_tracks})"))?,
        memory_cues: number(conn, "djmdCue", &format!("SELECT COUNT(*) FROM djmdCue WHERE rb_local_deleted = 0 AND Kind = 0 AND ContentID IN ({live_tracks})"))?,
        // Attribute 1 is a category of tags; 0 is a tag.
        my_tags: number(conn, "djmdMyTag", "SELECT COUNT(*) FROM djmdMyTag WHERE rb_local_deleted = 0 AND Attribute = 0")?,
        // Attribute 1 is a year or month folder; 0 is a session.
        history_sessions: number(conn, "djmdHistory", "SELECT COUNT(*) FROM djmdHistory WHERE rb_local_deleted = 0 AND Attribute = 0")?,
        analysis_files: 0,
        artwork_files: 0,
    })
}

impl Summary {
    /// Joins database counts with the measured trees.
    pub fn new(created_at: u64, app_version: Option<String>, measured: Measurement, counts: Counts) -> Self {
        let mut sizes = measured.sizes;
        sizes.updated_at = created_at;
        sizes.track_count = u32::try_from(counts.tracks).unwrap_or(u32::MAX);
        Self {
            version: VERSION,
            created_at,
            app_version,
            sizes,
            counts: Counts { analysis_files: measured.analysis_files, artwork_files: measured.artwork_files, ..counts },
        }
    }

    /// The summary an archive carries, if it has one this version understands.
    pub fn read(path: &Path) -> Result<Option<Self>> {
        Self::read_from(&mut archive::open(path)?)
    }

    /// As [`Summary::read`], from an archive already open.
    pub fn read_from(zip: &mut archive::Archive) -> Result<Option<Self>> {
        let Some(bytes) = archive::read_small(zip, NAME, MAX_BYTES)? else {
            return Ok(None);
        };
        let summary: Self = serde_json::from_slice(&bytes)?;
        Ok((summary.version == VERSION).then_some(summary))
    }
}

/// The library as it is now.
pub fn of_library(location: &rbl_db::LibraryLocation) -> Result<Summary> {
    let measured = sizes::measure_paths(&location.master_db, &crate::analysis_dir(location), &crate::artwork_dir(location))?;
    let db = rbl_db::Library::open(location.clone(), rbl_db::OpenMode::ReadOnly)?;
    let counts = count(db.connection())?;
    Ok(Summary::new(rbl_core::time::unix_millis(), None, measured, counts))
}

/// Works out the summary of an archive that lacks one by reading all of it:
/// every analysis file's section headers, and the database's counts from a
/// copy unpacked under `scratch` with `passphrase`. Takes seconds for a
/// large library, spread over the available cores.
pub fn from_archive(path: &Path, passphrase: &str, scratch: &Path) -> Result<Summary> {
    let manifest = Manifest::read(path)?;
    let mut zip = archive::open(path)?;
    let names = archive::names(&zip);
    let mut measured = Measurement::default();
    let (mut database, mut analysis) = (Vec::new(), Vec::new());
    for (index, name) in names.iter().enumerate() {
        if name.ends_with('/') {
            continue;
        }
        match archive::classify(name) {
            Some(Entry::Database | Entry::DatabaseWal) => database.push(index),
            Some(Entry::Analysis(_)) => analysis.push(index),
            // Sizes only: the headers say, without reading the contents.
            Some(Entry::LibraryFile(_)) => measured.sizes.database += zip.by_index_raw(index)?.size(),
            Some(Entry::Artwork(_)) => {
                measured.sizes.artwork += zip.by_index_raw(index)?.size();
                measured.artwork_files += 1;
            }
            Some(Entry::Other) => {}
            None => return Err(refused("The backup has an unsafe entry.")),
        }
    }
    drop(zip);
    fs::create_dir_all(scratch)?;
    let unpacked = tempfile::Builder::new().prefix(".summary-").tempdir_in(scratch)?;
    archive::read_each(
        path,
        &database,
        &|position, _, contents, report| {
            let mut output = fs::File::create(unpacked.path().join(&names[database[position]]))?;
            archive::copy(contents, &mut output, report).map(|_| ())
        },
        &mut |_, bytes| {
            measured.sizes.database += bytes;
            Ok(())
        },
    )?;
    let master_db = unpacked.path().join("master.db");
    if !master_db.is_file() {
        return Err(refused("The backup has no library database."));
    }
    measured.analysis_files = analysis.len() as u64;
    let (counts, analysis_sizes) = std::thread::scope(|scope| {
        let counts = scope.spawn(|| -> Result<Counts> {
            let location = rbl_db::LibraryLocation {
                master_db: master_db.clone(),
                share_root: unpacked.path().join("share"),
                passphrase: passphrase.to_owned(),
                is_real_install: false,
            };
            let db = rbl_db::Library::open(location, rbl_db::OpenMode::ReadOnly)?;
            Ok(count(db.connection())?)
        });
        let total = Mutex::new(BackupSizes::default());
        let measured = archive::read_each(
            path,
            &analysis,
            &|_, meta, contents, _| {
                let sizes = sizes::analysis_sizes(&mut sizes::Forward::new(contents), meta.size)?;
                total.lock().map_err(|_| refused("Reading the backup's analysis stopped unexpectedly."))?.add(&sizes);
                Ok(())
            },
            &mut |_, _| Ok(()),
        );
        let counts = counts.join().unwrap_or_else(|_| Err(refused("Reading the backup's database stopped unexpectedly.")));
        (counts, measured.and_then(|()| total.into_inner().map_err(|_| refused("Reading the backup's analysis stopped unexpectedly."))))
    });
    measured.sizes.add(&analysis_sizes?);
    Ok(Summary::new(manifest.created_at, None, measured, counts?))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::testing;

    fn add_cues(location: &rbl_db::LibraryLocation) {
        let db = rbl_db::Library::open(location.clone(), rbl_db::OpenMode::ReadWrite).unwrap();
        let conn = db.connection();
        let track = rbl_db::fixture::track_id(1);
        for (id, kind, deleted) in [("c1", 0, 0), ("c2", 0, 0), ("c3", 1, 0), ("c4", 5, 0), ("c5", 17, 0), ("c6", 4, 0), ("c7", 2, 1)] {
            conn.execute(
                "INSERT INTO djmdCue (ID, ContentID, InMsec, Kind, rb_local_deleted, created_at, updated_at) VALUES (?1, ?2, 0, ?3, ?4, '', '')",
                rusqlite::params![id, track, kind, deleted],
            )
            .unwrap();
        }
        conn.execute("UPDATE djmdContent SET AnalysisDataPath = '/PIONEER/USBANLZ/a/ANLZ0000.DAT' WHERE ID = ?1", [&track]).unwrap();
    }

    #[test]
    fn counts_live_rows_of_each_kind() {
        let dir = tempfile::tempdir().unwrap();
        let location = testing::library(dir.path());
        add_cues(&location);
        let db = rbl_db::Library::open(location.clone(), rbl_db::OpenMode::ReadOnly).unwrap();
        let counts = count(db.connection()).unwrap();
        let shape = rbl_db::fixture::Shape::default();
        assert_eq!(counts.tracks, shape.tracks as u64);
        assert_eq!(counts.playlists, shape.playlists as u64);
        assert_eq!(counts.history_sessions, shape.history_sessions as u64);
        assert_eq!((counts.memory_cues, counts.hot_cues), (2, 3));
        assert_eq!(counts.analyzed_tracks, 1);
    }

    #[test]
    fn an_archive_without_a_summary_is_measured_like_the_library_it_came_from() {
        let dir = tempfile::tempdir().unwrap();
        let location = testing::library(dir.path());
        add_cues(&location);
        let track = crate::analysis_dir(&location).join("P001/0001");
        fs::create_dir_all(&track).unwrap();
        fs::write(track.join("ANLZ0000.DAT"), testing::anlz(&[b"PQTZ", b"PWAV", b"PCOB"])).unwrap();
        fs::write(track.join("ANLZ0000.EXT"), testing::anlz(&[b"PWV3", b"PCO2", b"PSSI"])).unwrap();
        let art = crate::artwork_dir(&location).join("abc");
        fs::create_dir_all(&art).unwrap();
        fs::write(art.join("artwork_m.jpg"), [1; 40]).unwrap();
        let path = dir.path().join("backup.zip");
        testing::archive(&location, &path, None);
        assert_eq!(Summary::read(&path).unwrap(), None);

        let scratch = dir.path().join("scratch");
        let summary = from_archive(&path, &location.passphrase, &scratch).unwrap();
        let live = of_library(&location).unwrap();
        assert_eq!(summary.counts, live.counts);
        assert_eq!(summary.created_at, 1_790_000_000_000);
        assert_eq!(summary.sizes.track_count, 40);
        let (a, b) = (&summary.sizes, &live.sizes);
        assert_eq!((a.waveforms, a.cues, a.beat_grids, a.phrases, a.artwork, a.other), (b.waveforms, b.cues, b.beat_grids, b.phrases, b.artwork, b.other));
        assert_eq!((summary.counts.analysis_files, summary.counts.artwork_files), (2, 1));
        assert!(a.database > 0);
        assert_eq!(fs::read_dir(&scratch).unwrap().count(), 0, "the unpacked database is removed");
    }

    #[test]
    fn a_saved_summary_is_read_back_as_written() {
        let dir = tempfile::tempdir().unwrap();
        let location = testing::library(dir.path());
        let written = of_library(&location).unwrap();
        let path = dir.path().join("backup.zip");
        testing::archive(&location, &path, Some(&written));
        assert_eq!(Summary::read(&path).unwrap(), Some(written));
    }
}
