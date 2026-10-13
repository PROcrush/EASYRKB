//! What a track's key is, from the string rekordbox stores.
//!
//! `djmdKey.ScaleName` is musical notation — `Am`, `F#m`, `Eb` — and that
//! string is the only usable thing in the row. `Seq` is not a position on the
//! circle of fifths and not an index of anything: read against the live
//! library, `Dm` and `Bm` both carry 23, `Cm` and `Bbm` both carry 12. So this
//! parses the name.
//!
//! Read read-only against the reference library with
//! `cargo run -p rbl-db --example keys`: 24 keys, 38,507 of the 38,681 tracks
//! carrying one, spelled with flats everywhere except `F#`. One row is Camelot
//! (`2A`, one track), which is why that notation is accepted as a fallback
//! rather than assumed away.
//!
//! Everything comes back as a pitch class and a mode, so two keys can be
//! compared whatever they were spelled as: `Db` and `C#` are the same key and
//! must answer the same, which is the whole point of parsing rather than
//! matching strings.

/// Major or minor. rekordbox stores no other mode, and neither does the
/// reference library's 38,507 keyed tracks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Major,
    Minor,
}

/// A key as something two tracks can be compared by.
///
/// `pitch` is the tonic as a pitch class, C being 0 and rising by semitone, so
/// enharmonics land on the same number: `Db` and `C#` are both 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    pub pitch: u8,
    pub mode: Mode,
}

/// The Camelot wheel's minor keys, 1A to 12A, as pitch classes.
///
/// 1A is A♭ minor and each step up is a fifth: the wheel is the circle of
/// fifths with the numbers put on it, so this is that circle from A♭.
const CAMELOT_MINOR: [u8; 12] = [8, 3, 10, 5, 0, 7, 2, 9, 4, 11, 6, 1];

impl Key {
    /// How many semitones from this key to `other`, as a signed distance no
    /// larger than six: the shortest way round the octave.
    ///
    /// Mode is not part of it. What a semitone shift can do is move the pitch;
    /// nothing about pitching a record turns a minor track major, so a caller
    /// that cares about mode has to look at it.
    #[must_use]
    pub fn semitones_to(self, other: Self) -> i8 {
        let raw = i16::from(other.pitch) - i16::from(self.pitch);
        let wrapped = raw.rem_euclid(12);
        let shortest = if wrapped > 6 { wrapped - 12 } else { wrapped };
        i8::try_from(shortest).unwrap_or(0)
    }

    /// The same key written the way rekordbox writes it: flats, except `F#`.
    #[must_use]
    pub fn name(self) -> String {
        const NAMES: [&str; 12] =
            ["C", "Db", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];
        let root = NAMES.get(usize::from(self.pitch % 12)).copied().unwrap_or("C");
        match self.mode {
            Mode::Major => root.to_owned(),
            Mode::Minor => format!("{root}m"),
        }
    }
}

/// Parses a `ScaleName`, or a Camelot code, into a key.
///
/// Returns `None` for anything it does not recognise rather than guessing:
/// a key that is wrong is worse than a key that is missing, because sync would
/// act on it.
#[must_use]
pub fn parse(name: &str) -> Option<Key> {
    let text = name.trim();
    if text.is_empty() {
        return None;
    }
    camelot(text).or_else(|| notation(text))
}

/// `6B`, `2A` — a number 1 to 12 and a letter. `A` is minor, `B` major.
fn camelot(text: &str) -> Option<Key> {
    let (digits, letter) = text.split_at(text.len().checked_sub(1)?);
    let minor = match letter {
        "A" | "a" => true,
        "B" | "b" => false,
        _ => return None,
    };
    let number: usize = digits.parse().ok()?;
    if !(1..=12).contains(&number) {
        return None;
    }
    let minor_pitch = *CAMELOT_MINOR.get(number - 1)?;
    Some(if minor {
        Key { pitch: minor_pitch, mode: Mode::Minor }
    } else {
        // The major key three semitones above its relative minor.
        Key { pitch: (minor_pitch + 3) % 12, mode: Mode::Major }
    })
}

/// `Am`, `F#m`, `Eb`, `C` — a letter, an optional accidental, an optional `m`.
fn notation(text: &str) -> Option<Key> {
    let mut chars = text.chars();
    let letter = chars.next()?;
    let mut pitch = match letter.to_ascii_uppercase() {
        'C' => 0_i16,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let mut rest = chars.as_str();
    // Accidentals, however many: `Bbb` is not in this library but it is not
    // ambiguous either, and refusing it would be an opinion rather than a
    // reading.
    while let Some(next) = rest.chars().next() {
        match next {
            '#' | '♯' => pitch += 1,
            'b' | '♭' if rest.len() > 1 || pitch_letter_only(rest) => pitch -= 1,
            _ => break,
        }
        rest = rest.get(next.len_utf8()..)?;
    }
    // An empty tail is major: `C` is C major, which is how the library spells
    // it and how every notation does.
    let mode = match rest {
        "" | "maj" | "major" | "Major" => Mode::Major,
        "m" | "min" | "minor" | "Minor" => Mode::Minor,
        _ => return None,
    };
    Some(Key { pitch: u8::try_from(pitch.rem_euclid(12)).unwrap_or(0), mode })
}

/// Whether a trailing `b` is a flat rather than the start of something else.
///
/// `Bb` ends in a flat; `Bbm` has one and then a mode. Nothing in this
/// notation follows a flat with a letter other than a mode, so a lone `b` at
/// the end is always the accidental.
fn pitch_letter_only(rest: &str) -> bool {
    rest.len() == 1
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, reason = "a test that cannot parse a key has failed")]
mod tests {
    use super::*;

    /// Every `ScaleName` the reference library actually holds, with the number
    /// of tracks on it — read read-only, `--example keys`.
    const LIBRARY: [(&str, u8, Mode); 25] = [
        ("Am", 9, Mode::Minor),
        ("Em", 4, Mode::Minor),
        ("Gm", 7, Mode::Minor),
        ("Fm", 5, Mode::Minor),
        ("Dm", 2, Mode::Minor),
        ("Cm", 0, Mode::Minor),
        ("F#m", 6, Mode::Minor),
        ("Abm", 8, Mode::Minor),
        ("Bm", 11, Mode::Minor),
        ("Ebm", 3, Mode::Minor),
        ("Bbm", 10, Mode::Minor),
        ("Dbm", 1, Mode::Minor),
        ("C", 0, Mode::Major),
        ("D", 2, Mode::Major),
        ("E", 4, Mode::Major),
        ("A", 9, Mode::Major),
        ("G", 7, Mode::Major),
        ("Eb", 3, Mode::Major),
        ("F", 5, Mode::Major),
        ("Bb", 10, Mode::Major),
        ("Db", 1, Mode::Major),
        ("Ab", 8, Mode::Major),
        ("B", 11, Mode::Major),
        ("F#", 6, Mode::Major),
        // The one Camelot row, on a single track.
        ("2A", 3, Mode::Minor),
    ];

    #[test]
    fn every_key_the_real_library_holds_parses() {
        for (name, pitch, mode) in LIBRARY {
            let key = parse(name).unwrap_or_else(|| panic!("{name} did not parse"));
            assert_eq!(key.pitch, pitch, "{name}");
            assert_eq!(key.mode, mode, "{name}");
        }
    }

    #[test]
    fn enharmonics_are_the_same_key() {
        // The whole reason for parsing rather than comparing strings.
        assert_eq!(parse("Db"), parse("C#"));
        assert_eq!(parse("Ebm"), parse("D#m"));
        assert_eq!(parse("F#"), parse("Gb"));
    }

    #[test]
    fn the_camelot_wheel_agrees_with_the_notation_for_all_twenty_four() {
        // 8A is A minor and 8B its relative major, C — walking the wheel a
        // fifth at a time has to land on the same keys the names do.
        let pairs = [
            ("1A", "Abm"), ("2A", "Ebm"), ("3A", "Bbm"), ("4A", "Fm"),
            ("5A", "Cm"), ("6A", "Gm"), ("7A", "Dm"), ("8A", "Am"),
            ("9A", "Em"), ("10A", "Bm"), ("11A", "F#m"), ("12A", "Dbm"),
            ("1B", "B"), ("2B", "F#"), ("3B", "Db"), ("4B", "Ab"),
            ("5B", "Eb"), ("6B", "Bb"), ("7B", "F"), ("8B", "C"),
            ("9B", "G"), ("10B", "D"), ("11B", "A"), ("12B", "E"),
        ];
        for (code, name) in pairs {
            assert_eq!(parse(code), parse(name), "{code} should be {name}");
        }
    }

    #[test]
    fn a_key_writes_itself_back_the_way_rekordbox_spells_it() {
        // Flats everywhere except F#, which is what the library holds.
        for (name, _, _) in LIBRARY {
            let Some(key) = parse(name) else { continue };
            if name == "2A" {
                assert_eq!(key.name(), "Ebm");
                continue;
            }
            assert_eq!(key.name(), name);
        }
    }

    #[test]
    fn the_distance_between_two_keys_is_the_short_way_round() {
        let c = parse("C").unwrap();
        let b = parse("B").unwrap();
        let f_sharp = parse("F#").unwrap();
        assert_eq!(c.semitones_to(b), -1, "C to B is down one, not up eleven");
        assert_eq!(b.semitones_to(c), 1);
        // Six either way; the tritone has no short way round, and picking the
        // positive one is a decision rather than an accident.
        assert_eq!(c.semitones_to(f_sharp), 6);
        assert_eq!(c.semitones_to(c), 0);
    }

    #[test]
    fn nothing_it_cannot_read_comes_back_as_a_key() {
        // A wrong key is worse than a missing one: sync would act on it.
        for junk in ["", "  ", "H", "Hm", "13A", "0A", "2C", "Amaj7", "x", "Ammm"] {
            assert_eq!(parse(junk), None, "{junk:?} should not parse");
        }
    }

    #[test]
    fn the_spellings_a_person_might_type_are_read_too() {
        assert_eq!(parse(" am "), parse("Am"));
        assert_eq!(parse("A minor"), None, "a space is not this notation");
        assert_eq!(parse("Amin"), parse("Am"));
        assert_eq!(parse("Cmaj"), parse("C"));
    }
}
