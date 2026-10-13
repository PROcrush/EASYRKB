//! Timestamp and identifier shapes, checked against real rekordbox values.
#![allow(clippy::pedantic, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use rbl_core::ids::{Rng, MAX_CONTENT_ID, MAX_PLAYLIST_ID};
use rbl_core::time::{format_utc, local_date, now};

#[test]
fn a_timestamp_matches_the_format_rekordbox_writes() {
    // 2026-02-24 00:53:12.292 UTC, a real updated_at from the reference library.
    assert_eq!(format_utc(1_771_894_392, 292), "2026-02-24 00:53:12.292 +00:00");
    assert_eq!(format_utc(0, 0), "1970-01-01 00:00:00.000 +00:00");
}

#[test]
fn dates_are_right_across_the_awkward_boundaries() {
    assert!(format_utc(1_709_164_800, 0).starts_with("2024-02-29"), "leap day");
    assert!(format_utc(1_709_251_200, 0).starts_with("2024-03-01"));
    // 2000 is a leap year despite being a century, so 29 February exists.
    assert!(format_utc(951_782_400, 0).starts_with("2000-02-29"), "2000 is a leap year");
    assert!(format_utc(951_868_800, 0).starts_with("2000-03-01"));
    // 1900 was not, so the day after 28 February is 1 March.
    assert!(format_utc(-2_203_977_600, 0).starts_with("1900-02-28"));
    assert!(format_utc(-2_203_891_200, 0).starts_with("1900-03-01"));
    assert!(format_utc(1_767_225_599, 999).starts_with("2025-12-31 23:59:59.999"));
    assert!(format_utc(1_767_225_600, 0).starts_with("2026-01-01 00:00:00.000"));
}

#[test]
fn the_local_date_is_a_date_within_a_day_of_utc() {
    let local = local_date();
    assert_eq!(local.len(), 10, "{local}");
    assert!(local.as_bytes()[4] == b'-' && local.as_bytes()[7] == b'-', "{local}");
    // Whatever the zone, the local day is the UTC day or one either side of it.
    let utc_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let candidates: Vec<String> = [-86_400, 0, 86_400]
        .iter()
        .map(|delta| format_utc(utc_secs + delta, 0)[..10].to_owned())
        .collect();
    assert!(candidates.contains(&local), "{local} is not near {candidates:?}");
}

#[test]
fn every_day_of_a_leap_year_round_trips() {
    // Walking a whole year catches an off-by-one that spot checks would miss.
    let mut day = 1_704_067_200_i64; // 2024-01-01
    let mut seen = Vec::new();
    for _ in 0..366 {
        seen.push(format_utc(day, 0)[..10].to_owned());
        day += 86_400;
    }
    assert_eq!(seen.first().map(String::as_str), Some("2024-01-01"));
    assert_eq!(seen.get(59).map(String::as_str), Some("2024-02-29"));
    assert_eq!(seen.last().map(String::as_str), Some("2024-12-31"));
    seen.dedup();
    assert_eq!(seen.len(), 366, "every day must be distinct");
}

#[test]
fn milliseconds_are_always_three_digits() {
    for (millis, expected) in [(0, ".000"), (7, ".007"), (70, ".070"), (999, ".999")] {
        assert!(format_utc(0, millis).contains(expected), "{millis}");
    }
    // A value that cannot happen is clamped rather than printed as four digits.
    assert!(format_utc(0, 1234).contains(".999"));
}

#[test]
fn now_has_the_shape_the_database_expects() {
    let stamp = now();
    assert_eq!(stamp.len(), "2026-02-24 00:53:12.292 +00:00".len(), "{stamp}");
    assert!(stamp.ends_with(" +00:00"), "{stamp}");
    assert_eq!(stamp.as_bytes().get(4), Some(&b'-'));
    assert_eq!(stamp.as_bytes().get(10), Some(&b' '));
    assert_eq!(stamp.as_bytes().get(19), Some(&b'.'));
}

#[test]
fn a_uuid_has_the_version_and_variant_bits_set() {
    let mut rng = Rng::from_seed(12345);
    for _ in 0..500 {
        let uuid = rng.uuid4();
        assert_eq!(uuid.len(), 36, "{uuid}");
        let parts: Vec<&str> = uuid.split('-').collect();
        assert_eq!(parts.iter().map(|p| p.len()).collect::<Vec<_>>(), vec![8, 4, 4, 4, 12]);
        assert!(uuid.chars().all(|c| c == '-' || (c.is_ascii_hexdigit() && !c.is_uppercase())));
        assert_eq!(parts.get(2).and_then(|p| p.chars().next()), Some('4'), "version: {uuid}");
        let variant = parts.get(3).and_then(|p| p.chars().next()).unwrap_or('x');
        assert!("89ab".contains(variant), "variant: {uuid}");
    }
}

#[test]
fn generated_ids_do_not_repeat_over_a_realistic_run() {
    let mut rng = Rng::from_seed(0xdead_beef);
    let mut uuids = std::collections::HashSet::new();
    let mut playlist_ids = std::collections::HashSet::new();
    for _ in 0..20_000 {
        uuids.insert(rng.uuid4());
        playlist_ids.insert(rng.numeric_id(MAX_PLAYLIST_ID));
    }
    assert_eq!(uuids.len(), 20_000, "uuids collided");
    // A 32-bit space and 20k draws: a few collisions are expected, which is
    // exactly why the writer checks the table before using an id.
    assert!(playlist_ids.len() > 19_900, "{} distinct", playlist_ids.len());
}

#[test]
fn numeric_ids_stay_inside_the_range_rekordbox_uses() {
    let mut rng = Rng::from_seed(7);
    for _ in 0..5_000 {
        let content: u64 = rng.numeric_id(MAX_CONTENT_ID).parse().unwrap();
        assert!((1..MAX_CONTENT_ID).contains(&content), "{content}");
        let playlist: u64 = rng.numeric_id(MAX_PLAYLIST_ID).parse().unwrap();
        assert!((1..MAX_PLAYLIST_ID).contains(&playlist), "{playlist}");
        assert!(!rng.numeric_id(MAX_PLAYLIST_ID).starts_with('0'), "no leading zeroes");
    }
}

#[test]
fn a_zero_seed_does_not_freeze_the_generator() {
    // Zero is a fixed point of xorshift; seeding with it must not produce a
    // constant stream.
    let mut rng = Rng::from_seed(0);
    let first = rng.next_u64();
    assert_ne!(first, rng.next_u64());
}

#[test]
fn two_generators_seeded_from_entropy_diverge() {
    // Made back to back, so the clock and the stack address are likely
    // identical: only the process-wide counter separates them.
    let mut streams = std::collections::HashSet::new();
    for _ in 0..64 {
        streams.insert(Rng::from_entropy().next_u64());
    }
    assert_eq!(streams.len(), 64, "generators made in the same tick collided");
}
