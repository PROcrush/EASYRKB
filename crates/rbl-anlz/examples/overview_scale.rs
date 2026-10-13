//! READ-ONLY: checks the scale the overview waveform is drawn at.
//!
//! `src/canvas/waveform.ts` stacks a half waveform as `low/128 + mid/256 +
//! high/128` of its band. Those divisors came from matching 1,200 painted
//! columns of a 2x capture against that track's own `PWV6`, column for column.
//! This prints what a track's data does against them, so the constants can be
//! re-checked against any track rather than taken on faith.
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

/// Full scale per band, mirroring `STACK_SCALE` in the renderer.
const SCALE: [f64; 3] = [128.0, 256.0, 128.0];

fn main() {
    let want = std::env::args().nth(1).unwrap_or_else(|| "Take Me Home".to_owned());
    let db = match rbl_db::Library::open_installed_read_only() {
        Ok(db) => db,
        Err(e) => {
            println!("cannot open library: {e}");
            return;
        }
    };
    let share = db.location().share_root.clone();
    let (library, _) = rbl_index::load(&db).expect("index");

    let mut seen = 0_u32;
    for row in 0..library.len() as u32 {
        let i = row as usize;
        if !library.title.get(i).contains(&want) {
            continue;
        }
        let dat = rbl_anlz::resolve(&share, library.analysis_path.get(i));
        let Ok(two) = rbl_anlz::Anlz::read(&rbl_anlz::sibling(&dat, "2EX")) else { continue };
        let Some((_, data)) = two.waveform(b"PWV6") else { continue };

        let mut bands = [0_u8; 3];
        let mut stacks: Vec<f64> = Vec::with_capacity(data.len() / 3);
        for c in data.chunks_exact(3) {
            for b in 0..3 {
                bands[b] = bands[b].max(c[b]);
            }
            stacks.push((0..3).map(|b| f64::from(c[b]) / SCALE[b]).sum());
        }
        if stacks.is_empty() {
            continue;
        }
        stacks.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let at = |p: f64| stacks[((stacks.len() as f64 * p) as usize).min(stacks.len() - 1)];
        // Clipping is expected at the top — the bands do not peak together, so
        // reserving headroom for a stack that never happens would draw every
        // waveform short. It is a problem only if it is more than a sliver.
        let clipped = stacks.iter().filter(|s| **s > 1.0).count();

        println!("{} — {}", library.title.get(i), library.artist_name(row));
        println!("  {} columns   per-band max {bands:?}", stacks.len());
        println!(
            "  band filled: p50 {:.2}  p90 {:.2}  p99 {:.2}  max {:.2}   clipped {:.2}%",
            at(0.5),
            at(0.9),
            at(0.99),
            at(1.0),
            clipped as f64 / stacks.len() as f64 * 100.0,
        );
        seen += 1;
        if seen == 10 {
            break;
        }
    }
    if seen == 0 {
        println!("no analysed track matching {want:?}");
    }
}
