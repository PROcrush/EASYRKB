//! Offline search aliases. Generated when metadata is indexed, never per keypress.
use std::{collections::HashMap, sync::OnceLock};
use pinyin::ToPinyin;
use serde::{Deserialize, Serialize};
use crate::{Library, NO_ID};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReadingOverride {
    pub original: String,
    pub reading: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrackOverride {
    pub title: Option<ReadingOverride>,
    pub artist: Option<ReadingOverride>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub title_pinyin: String,
    pub artist_pinyin: String,
    pub title_initials: String,
    pub artist_initials: String,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Aliases {
    pub plain: String,
    pub spaced: String,
    pub initials: String,
}

fn phrases() -> &'static HashMap<&'static str, &'static str> {
    static PHRASES: OnceLock<HashMap<&str, &str>> = OnceLock::new();
    PHRASES.get_or_init(|| include_str!("../data/phrases.tsv").lines().filter_map(|line| line.split_once('\t')).collect())
}

/// ASCII words are kept whole (DJ/Remix/Live and digits); only Chinese syllables
/// are shortened to their first letter. The same aliases work with ü and v.
#[must_use]
pub fn aliases(text: &str, correction: Option<&ReadingOverride>) -> Aliases {
    let mut syllables: Vec<(String, bool)> = Vec::new();
    if let Some(custom) = correction.filter(|custom| custom.original == text && !custom.reading.is_empty()) {
        let ascii_words: Vec<String> = text.split(|ch: char| !ch.is_ascii_alphanumeric()).filter(|word| !word.is_empty()).map(str::to_ascii_lowercase).collect();
        syllables.extend(custom.reading.split_whitespace().map(|word| {
            let word = word.to_lowercase().replace('ü', "v");
            let chinese = !ascii_words.contains(&word) && !word.chars().all(|ch| ch.is_ascii_digit());
            (word, chinese)
        }));
    } else if text.chars().any(|ch| ch.to_pinyin().is_some()) {
        let bounds: Vec<usize> = text.char_indices().map(|(i, _)| i).chain(std::iter::once(text.len())).collect();
        let mut at = 0;
        let mut ascii = String::new();
        while at + 1 < bounds.len() {
            let rest = text.get(bounds[at]..).unwrap_or("");
            let ch = rest.chars().next().unwrap_or(' ');
            if let Some(reading) = ch.to_pinyin() {
                if !ascii.is_empty() { syllables.push((std::mem::take(&mut ascii), false)); }
                let matched = ((at + 2)..bounds.len().min(at + 17)).rev().find_map(|end| {
                    phrases().get(text.get(bounds[at]..bounds[end])?).map(|value| (end, *value))
                });
                if let Some((end, value)) = matched {
                    syllables.extend(value.split_whitespace().map(|word| (word.into(), true)));
                    at = end;
                } else {
                    syllables.push((reading.plain().replace('ü', "v"), true));
                    at += 1;
                }
            } else {
                if ch.is_ascii_alphanumeric() { ascii.push(ch.to_ascii_lowercase()); }
                else if !ascii.is_empty() { syllables.push((std::mem::take(&mut ascii), false)); }
                at += 1;
            }
        }
        if !ascii.is_empty() { syllables.push((ascii, false)); }
    }
    Aliases {
        plain: syllables.iter().map(|(s, _)| s.as_str()).collect(),
        spaced: syllables.iter().map(|(s, _)| s.as_str()).collect::<Vec<_>>().join(" "),
        initials: syllables.iter().map(|(s, chinese)| if *chinese { s.get(..1).unwrap_or("") } else { s }).collect(),
    }
}

#[must_use]
pub fn search_value(original: &str, correction: Option<&ReadingOverride>) -> String {
    let names = aliases(original, correction);
    // fold() strips the diaeresis for ordinary Latin collation. Also index the
    // u spelling so both lü and lv input match without changing Latin searches.
    format!("{original} {} {} {} {}", names.plain, names.spaced, names.initials, names.plain.replace('v', "u"))
}

impl Library {
    /// Corrections are scoped to the library and original text. A renamed song
    /// gets new automatic aliases instead of inheriting its old pronunciation.
    pub fn apply_pinyin_overrides(&mut self, values: HashMap<u64, TrackOverride>) {
        if values.is_empty() && self.pinyin_overrides.is_empty() { return; }
        self.pinyin_overrides = values;
        self.build_search();
    }
    pub fn rebuild_pinyin(&mut self, ids: Option<&[String]>, mut progress: impl FnMut(usize, usize)) -> (usize, usize) {
        let rows: Vec<usize> = ids.map_or_else(|| (0..self.len()).collect(), |ids| ids.iter().filter_map(|id| self.row_of(id).map(|row| row as usize)).collect());
        let mut chinese = 0;
        let mut search = HashMap::new();
        progress(0, rows.len());
        for (done, &row) in rows.iter().enumerate() {
            if self.title.get(row).chars().chain(self.artists.name(self.artist.get(row).copied().unwrap_or(NO_ID)).chars()).any(|ch| ch.to_pinyin().is_some()) { chinese += 1; }
            search.insert(row, self.search_text(row));
            if (done + 1) % 256 == 0 { progress(done + 1, rows.len()); }
        }
        self.search.replace_rows(&search);
        progress(rows.len(), rows.len());
        (rows.len(), chinese)
    }
    #[must_use]
    pub fn pinyin_reading(&self, id: &str) -> Option<Reading> {
        let row = self.row_of(id)? as usize;
        let title = self.title.get(row);
        let artist = self.artists.name(self.artist.get(row).copied().unwrap_or(NO_ID));
        let custom = self.ids.get(row).and_then(|id| self.pinyin_overrides.get(id));
        let title_names = aliases(title, custom.and_then(|custom| custom.title.as_ref()));
        let artist_names = aliases(artist, custom.and_then(|custom| custom.artist.as_ref()));
        Some(Reading { id: id.into(), title: title.into(), artist: artist.into(), title_pinyin: title_names.spaced, artist_pinyin: artist_names.spaced, title_initials: title_names.initials, artist_initials: artist_names.initials })
    }
}
