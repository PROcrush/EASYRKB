//! READ-ONLY: checks the `PSSI` phrase labels and `PVDI` vocal strip against
//! the installed library, which is how both were worked out in the first place.
//! `cargo run --release -p rbl-anlz --example phrases -- "<title fragment>"`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::collections::BTreeMap;

fn main() {
    let want = std::env::args().nth(1).unwrap_or_default();
    let limit: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(2000);
    let db = rbl_db::Library::open_installed_read_only().expect("open db");
    let share = db.location().share_root.clone();
    let (library, _) = rbl_index::load(&db).expect("index");

    let mut labels: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut empty = 0usize;
    let mut with_pssi = 0usize;
    let mut fills = 0usize;
    let mut max_phrases = 0usize;
    let mut with_pvdi = 0usize;
    let mut seen = 0usize;

    for row in 0..library.len() as u32 {
        let i = row as usize;
        if library.analysis_path.get(i).is_empty() { continue }
        let title = library.title.get(i);
        let dat = rbl_anlz::resolve(&share, library.analysis_path.get(i));
        if let Ok(a) = rbl_anlz::Anlz::read(&rbl_anlz::sibling(&dat, "EXT")) {
            if let Some(st) = a.song_structure() {
                with_pssi += 1;
                max_phrases = max_phrases.max(st.phrases.len());
                for p in &st.phrases {
                    *labels.entry(p.label).or_default() += 1;
                    if p.label.is_empty() { empty += 1 }
                    if p.fill_in { fills += 1 }
                }
                if !want.is_empty() && title.contains(&want) {
                    println!("{title} — mood {:?} end_beat {} bank {}", st.mood, st.end_beat, st.bank);
                    println!("  {}", st.phrases.iter().map(|p| p.label).collect::<Vec<_>>().join(", "));
                }
            }
        }
        if let Ok(a) = rbl_anlz::Anlz::read(&rbl_anlz::sibling(&dat, "2EX")) {
            if a.vocals().is_some() { with_pvdi += 1 }
        }
        seen += 1;
        if seen >= limit { break }
    }
    println!("\nscanned {seen}: {with_pssi} with PSSI, {with_pvdi} with PVDI");
    println!("max phrases in one track: {max_phrases}");
    println!("phrases with a fill-in: {fills}");
    println!("unlabelled phrases: {empty}");
    println!("labels: {labels:?}");
}
