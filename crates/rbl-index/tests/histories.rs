//! Reading the history tree out of a real-schema library.
//!
//! Histories are the same two-table shape as playlists — a tree of named rows
//! with a parent, and a membership table in `TrackNo` order — so these check
//! the shared reader against the half of it that has no writer: everything
//! here is what rekordbox recorded before the library was opened.
#![allow(clippy::pedantic, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use rbl_db::fixture::{self, history_id, Shape};
use rbl_db::{Library as Db, OpenMode};
use rbl_index::{SortColumn, TrackSource, ViewSpec};

struct Fixture {
    _dir: tempfile::TempDir,
    library: rbl_index::Library,
}

fn open(shape: Shape) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let location = fixture::build(dir.path(), shape).expect("build the fixture");
    let db = Db::open(location, OpenMode::ReadOnly).expect("open read-only");
    let (library, _stats) = rbl_index::load(&db).expect("index");
    Fixture { _dir: dir, library }
}

fn spec(source: TrackSource) -> ViewSpec {
    ViewSpec { source, sort: SortColumn::TrackNo, descending: false, query: String::new(), filter: Default::default() }
}

#[test]
fn the_sessions_and_the_folders_they_are_filed_under_are_all_read() {
    let f = open(Shape::default());
    let histories = f.library.histories();
    // Two sessions, two month folders and a year folder: rekordbox files
    // sessions under a folder per year and per month, and all five are rows
    // in the same table told apart by `Attribute`.
    assert_eq!(histories.len(), 5);
    let names: Vec<&str> = (0..histories.len()).map(|i| histories.name(i)).collect();
    assert!(names.contains(&"2026"), "{names:?}");
    assert!(names.contains(&"HISTORY 2026-09-01"), "{names:?}");
}

#[test]
fn a_session_opens_on_the_tracks_it_played_in_the_order_it_played_them() {
    let f = open(Shape::default());
    let index = f.library.histories().index_of(history_id(0).parse().unwrap()).unwrap();
    let view = f.library.open_view(&spec(TrackSource::History(index)));
    assert_eq!(view.len(), 5);
    let titles: Vec<&str> =
        view.rows.iter().map(|&r| f.library.title.get(r as usize)).collect();
    assert_eq!(titles, ["Track 000", "Track 001", "Track 002", "Track 003", "Track 004"]);
}

#[test]
fn a_year_folder_holds_no_tracks_of_its_own() {
    // Selecting one is not an error and not the whole collection: a folder
    // holds sessions, so it opens on nothing.
    let f = open(Shape::default());
    let index = f.library.histories().index_of(2026).unwrap();
    assert_eq!(f.library.open_view(&spec(TrackSource::History(index))).len(), 0);
}

#[test]
fn the_month_is_filed_under_the_year_and_the_sessions_under_the_month() {
    let f = open(Shape::default());
    let histories = f.library.histories();
    let year = histories.index_of(2026).unwrap();
    let month = histories.index_of(202_609).unwrap();
    let session = histories.index_of(history_id(1).parse().unwrap()).unwrap();
    assert_eq!(histories.parent[year], rbl_index::NO_ID, "the year is a root");
    assert_eq!(histories.parent[month] as usize, year);
    assert_eq!(histories.parent[session] as usize, month);
}

#[test]
fn a_library_that_has_never_recorded_one_reads_as_an_empty_section() {
    // Not an error. A history table with no sessions, and a schema without the
    // tables at all, both have to open: the required-column probe does not
    // insist on them, so nothing else would catch it.
    let f = open(Shape { history_sessions: 0, ..Shape::default() });
    assert!(f.library.histories().is_empty());
}

#[test]
fn a_view_of_a_history_index_that_does_not_exist_is_empty_rather_than_a_panic() {
    let f = open(Shape::default());
    assert_eq!(f.library.open_view(&spec(TrackSource::History(9_999))).len(), 0);
}
