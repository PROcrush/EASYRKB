//! Ordering musical keys.
//!
//! Two orders, because rekordbox sorts the Key column by what it shows: the
//! classic names alphabetically (`A`, `Ab`, `Abm`, `Am`, `B`, …) and the
//! alphanumeric ones round the Camelot wheel (`1A`, `1B`, `2A`, …). The
//! general string fold drops `#` as punctuation, which put `F` and `F#` on
//! top of each other; keys are compared by their own rule instead.

use std::cmp::Ordering;

/// The wheel, `1A` to `12A` and `1B` to `12B`, as rekordbox spells the keys.
const MINOR: [&str; 12] = ["Abm", "Ebm", "Bbm", "Fm", "Cm", "Gm", "Dm", "Am", "Em", "Bm", "F#m", "Dbm"];
const MAJOR: [&str; 12] = ["B", "F#", "Db", "Ab", "Eb", "Bb", "F", "C", "G", "D", "A", "E"];

/// The enharmonic spellings a library can hold, onto the wheel's own.
fn canonical(name: &str) -> &str {
    match name.trim() {
        "G#m" => "Abm",
        "D#m" => "Ebm",
        "A#m" => "Bbm",
        "C#m" => "Dbm",
        "Gbm" => "F#m",
        "Gb" => "F#",
        "C#" => "Db",
        "G#" => "Ab",
        "D#" => "Eb",
        "A#" => "Bb",
        other => other,
    }
}

/// Where a key sits on the Camelot wheel: `1A` is 0, `1B` is 1, `2A` is 2,
/// and so on to `12B` at 23. A name that is not a key — blank, or one the
/// wheel does not know — is `u32::MAX`, so it sorts after every key.
#[must_use]
pub fn camelot_rank(name: &str) -> u32 {
    let name = canonical(name);
    if let Some(i) = MINOR.iter().position(|k| *k == name) {
        return u32::try_from(i * 2).unwrap_or(u32::MAX);
    }
    if let Some(i) = MAJOR.iter().position(|k| *k == name) {
        return u32::try_from(i * 2 + 1).unwrap_or(u32::MAX);
    }
    // A Camelot code stored as the key itself: some libraries hold `8A`.
    let code = name.as_bytes();
    if let Some((&letter, digits)) = code.split_last() {
        if (letter == b'A' || letter == b'B' || letter == b'a' || letter == b'b') && !digits.is_empty() {
            if let Ok(n) = std::str::from_utf8(digits).unwrap_or("").parse::<u32>() {
                if (1..=12).contains(&n) {
                    return (n - 1) * 2 + u32::from(letter.eq_ignore_ascii_case(&b'B'));
                }
            }
        }
    }
    u32::MAX
}

/// Alphabetical, case-insensitive, and `#` counts: `F` before `F#` before
/// `F#m` before `Fm`. Blank last, as a missing value goes to the end.
#[must_use]
pub fn cmp_names(left: &str, right: &str) -> Ordering {
    match (left.is_empty(), right.is_empty()) {
        (true, true) => return Ordering::Equal,
        (true, false) => return Ordering::Greater,
        (false, true) => return Ordering::Less,
        (false, false) => {}
    }
    let mut lhs = left.bytes().map(|byte| byte.to_ascii_lowercase());
    let mut rhs = right.bytes().map(|byte| byte.to_ascii_lowercase());
    loop {
        match (lhs.next(), rhs.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(l), Some(r)) if l == r => {}
            (Some(l), Some(r)) => return l.cmp(&r),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn the_wheel_goes_round_from_1a() {
        assert_eq!(camelot_rank("Abm"), 0);
        assert_eq!(camelot_rank("B"), 1);
        assert_eq!(camelot_rank("Ebm"), 2);
        assert_eq!(camelot_rank("E"), 23);
        assert_eq!(camelot_rank("G#m"), camelot_rank("Abm"), "an enharmonic spelling is the same key");
        assert_eq!(camelot_rank("8A"), camelot_rank("Am"), "a code stored as a key");
        assert_eq!(camelot_rank(""), u32::MAX);
        assert_eq!(camelot_rank("Unknown"), u32::MAX);
    }

    #[test]
    fn the_names_keep_their_sharps_apart() {
        let mut keys = vec!["Fm", "F#m", "F", "F#", "", "Ab", "A", "am"];
        keys.sort_by(|a, b| cmp_names(a, b));
        assert_eq!(keys, ["A", "Ab", "am", "F", "F#", "F#m", "Fm", ""]);
    }
}
