//! The track filter: each column alone, the columns together, and the value
//! lists the bar offers.
#![allow(clippy::pedantic, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use rbl_index::cache::{decode, encode, Fingerprint};
use rbl_index::testing::{add_playlist, library_from, set_my_tags, TestTrack};
use rbl_index::{BpmFilter, SortColumn, TagCategory, TrackFilter, TrackSource, ViewSpec};

fn track(id: u64, bpm: u32, key: &'static str, rating: u8, color: u8) -> TestTrack {
    TestTrack { id, title: "t", artist: "a", bpm_x100: bpm, key, rating, color, ..TestTrack::default() }
}

/// Six tracks: two at 128 (one of them 127.98), one each at 126, 130, 140,
/// and one unanalysed.
fn sample() -> Vec<TestTrack> {
    vec![
        track(1, 12800, "Fm", 5, 2),  // 4A, Red
        track(2, 12798, "A", 3, 0),   // 11B
        track(3, 12600, "Abm", 0, 5), // 1A, Green
        track(4, 13000, "A", 3, 2),   // 11B again, under its own id
        track(5, 14000, "F#", 1, 0),  // 2B
        track(6, 0, "", 0, 0),
    ]
}

fn spec(filter: TrackFilter) -> ViewSpec {
    ViewSpec {
        source: TrackSource::Collection,
        sort: SortColumn::TrackNo,
        descending: false,
        query: String::new(),
        filter,
    }
}

fn ids(lib: &rbl_index::Library, filter: TrackFilter) -> Vec<u64> {
    lib.open_view(&spec(filter)).rows.iter().map(|&r| lib.ids[r as usize]).collect()
}

fn bpm(values: &[u32], pct: u8, master: Option<u32>) -> TrackFilter {
    TrackFilter {
        bpm: Some(BpmFilter { values: values.to_vec(), tolerance_pct: pct, master_bpm_x100: master }),
        ..TrackFilter::default()
    }
}

#[test]
fn an_empty_filter_changes_nothing() {
    let lib = library_from(&sample());
    assert!(TrackFilter::default().is_empty());
    assert_eq!(ids(&lib, TrackFilter::default()), [1, 2, 3, 4, 5, 6]);
}

#[test]
fn a_picked_bpm_is_its_whole_number_bucket() {
    let lib = library_from(&sample());
    // 127.98 rounds to 128, so it sits in the same bucket the list showed it under.
    assert_eq!(ids(&lib, bpm(&[128], 0, None)), [1, 2]);
    assert_eq!(ids(&lib, bpm(&[128, 140], 0, None)), [1, 2, 5]);
    assert_eq!(ids(&lib, bpm(&[999], 0, None)), Vec::<u64>::new());
}

#[test]
fn a_tolerance_widens_a_picked_bpm() {
    let lib = library_from(&sample());
    // 128 ± 2% is 125.44..130.56: takes in 126 and 130, not 140.
    assert_eq!(ids(&lib, bpm(&[128], 2, None)), [1, 2, 3, 4]);
    // ±1% is 126.72..129.28: 126 and 130 fall out again.
    assert_eq!(ids(&lib, bpm(&[128], 1, None)), [1, 2]);
}

#[test]
fn all_with_a_master_player_is_a_band_around_its_bpm() {
    let lib = library_from(&sample());
    // ±0% around 128.00 is exactly 128.00: 127.98 is not it.
    assert_eq!(ids(&lib, bpm(&[], 0, Some(12800))), [1]);
    assert_eq!(ids(&lib, bpm(&[], 2, Some(12800))), [1, 2, 3, 4]);
    // 133 ± 6% is 125.02..140.98: everything analysed.
    assert_eq!(ids(&lib, bpm(&[], 6, Some(13300))), [1, 2, 3, 4, 5]);
}

#[test]
fn all_without_a_master_player_is_every_track() {
    let lib = library_from(&sample());
    // The ± list is inert with nothing to centre on; a ticked BPM column then
    // matches everything, unanalysed tracks included.
    assert_eq!(ids(&lib, bpm(&[], 3, None)), [1, 2, 3, 4, 5, 6]);
}

#[test]
fn keys_match_by_every_id_carrying_the_name() {
    let lib = library_from(&sample());
    // Two tracks say `A`, each under its own interner id, as djmdKey does.
    let a = lib.key_ids_named(&["A".to_owned()]);
    assert_eq!(a.len(), 2);
    let filter = TrackFilter { keys: Some(a), ..TrackFilter::default() };
    assert_eq!(ids(&lib, filter), [2, 4]);

    let none = TrackFilter { keys: Some(Vec::new()), ..TrackFilter::default() };
    assert_eq!(ids(&lib, none), Vec::<u64>::new());
    assert!(lib.key_ids_named(&["Zm".to_owned()]).is_empty());
}

#[test]
fn ratings_and_colours_are_sets() {
    let lib = library_from(&sample());
    let stars = TrackFilter { ratings: Some(vec![0, 5]), ..TrackFilter::default() };
    assert_eq!(ids(&lib, stars), [1, 3, 6]);
    let red = TrackFilter { colors: Some(vec![2]), ..TrackFilter::default() };
    assert_eq!(ids(&lib, red), [1, 4]);
    let red_or_green = TrackFilter { colors: Some(vec![2, 5]), ..TrackFilter::default() };
    assert_eq!(ids(&lib, red_or_green), [1, 3, 4]);
    // A colour id past the eight is not a panic, just a miss.
    let odd = TrackFilter { colors: Some(vec![200]), ..TrackFilter::default() };
    assert_eq!(ids(&lib, odd), Vec::<u64>::new());
}

#[test]
fn ticked_columns_combine_with_and() {
    let lib = library_from(&sample());
    let filter = TrackFilter {
        bpm: Some(BpmFilter { values: vec![128, 130], tolerance_pct: 0, master_bpm_x100: None }),
        keys: Some(lib.key_ids_named(&["A".to_owned()])),
        ratings: Some(vec![3]),
        colors: Some(vec![2]),
    };
    assert_eq!(ids(&lib, filter), [4]);
}

#[test]
fn the_filter_applies_after_the_query_and_within_a_playlist() {
    let mut lib = library_from(&sample());
    let pl = add_playlist(&mut lib, "Set", &[4, 0, 2]);
    let view = lib.open_view(&ViewSpec {
        source: TrackSource::Playlist(pl),
        sort: SortColumn::TrackNo,
        descending: false,
        query: String::new(),
        filter: bpm(&[128, 140], 0, None),
    });
    // Membership order kept: track 5 then track 1.
    let got: Vec<u64> = view.rows.iter().map(|&r| lib.ids[r as usize]).collect();
    assert_eq!(got, [5, 1]);
}

#[test]
fn the_value_lists_count_the_unfiltered_list_in_the_bars_order() {
    let mut lib = library_from(&sample());
    set_my_tags(
        &mut lib,
        vec![TagCategory { name: "Lexicon Tags".into(), tags: vec!["Components ▶ Synth".into()] }],
    );
    // A filter that would leave one track must not shrink the lists.
    let values = lib.filter_values(&spec(bpm(&[140], 0, None)));
    let bpms: Vec<(u32, u32)> = values.bpms.iter().map(|c| (c.value, c.count)).collect();
    assert_eq!(bpms, [(126, 1), (128, 2), (130, 1), (140, 1)]);
    // Camelot order: 1A Abm, 2B F#, 4A Fm, 11B A — and `A` is one entry with
    // both ids' tracks behind it.
    let keys: Vec<(&str, u32)> = values.keys.iter().map(|c| (c.value.as_str(), c.count)).collect();
    assert_eq!(keys, [("Abm", 1), ("F#", 1), ("Fm", 1), ("A", 2)]);
    assert_eq!(values.tags.len(), 1);
    assert_eq!(values.tags[0].tags, ["Components ▶ Synth"]);
}

#[test]
fn the_value_lists_follow_the_source_and_query() {
    let mut lib = library_from(&sample());
    let pl = add_playlist(&mut lib, "Set", &[2, 4]);
    let values = lib.filter_values(&ViewSpec {
        source: TrackSource::Playlist(pl),
        sort: SortColumn::Title,
        descending: true,
        query: String::new(),
        filter: TrackFilter::default(),
    });
    let bpms: Vec<u32> = values.bpms.iter().map(|c| c.value).collect();
    assert_eq!(bpms, [126, 140]);
    let keys: Vec<&str> = values.keys.iter().map(|c| c.value.as_str()).collect();
    assert_eq!(keys, ["Abm", "F#"]);
}

#[test]
fn a_snapshot_keeps_the_tag_categories() {
    let mut lib = library_from(&sample());
    set_my_tags(
        &mut lib,
        vec![
            TagCategory { name: "Lexicon Tags".into(), tags: vec!["Synth".into(), "Vocal".into()] },
            TagCategory { name: "Empty Category".into(), tags: Vec::new() },
        ],
    );
    let fp = Fingerprint {
        format: rbl_index::cache::FORMAT,
        db_len: 1,
        db_modified_ns: 2,
        wal_len: 3,
        wal_modified_ns: 4,
        db_version: 6000,
        content: 5,
        database: 6,
    };
    let restored = decode(&encode(&lib, fp), fp).expect("decodes");
    assert_eq!(restored.my_tags(), lib.my_tags());
    assert_eq!(restored.color, lib.color);
}
