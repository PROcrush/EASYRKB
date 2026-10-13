//! READ-ONLY: dumps a track's colour waveform tags, to work out how rekordbox
//! turns their six channels into the picture it draws.
//! `cargo run --release -p rbl-anlz --example colourwave -- "<title fragment>"`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

fn main() {
    let want = std::env::args().nth(1).unwrap_or_else(|| "Take Me Home".to_owned());
    let db = match rbl_db::Library::open_installed_read_only() {
        Ok(db) => db,
        Err(e) => { println!("cannot open library: {e}"); return; }
    };
    let share = db.location().share_root.clone();
    let (library, _) = rbl_index::load(&db).expect("index");

    let Some(row) = (0..library.len() as u32)
        .find(|&r| library.title.get(r as usize).contains(&want))
    else {
        println!("no track matching {want:?}");
        return;
    };
    let i = row as usize;
    println!("{}  —  {}", library.title.get(i), library.artist_name(row));

    let dat = rbl_anlz::resolve(&share, library.analysis_path.get(i));
    for extension in ["EXT", "2EX"] {
        let path = rbl_anlz::sibling(&dat, extension);
        let Ok(anlz) = rbl_anlz::Anlz::read(&path) else { continue };
        println!("\n{}:", path.display());
        for section in &anlz.sections {
            println!("  {}  header {}  payload {}", section.tag, section.header.len(), section.payload.len());
        }
        for tag in [b"PWV4", b"PWV5", b"PWV6", b"PWV7"] {
            let Some(section) = anlz.section(tag) else { continue };
            let header = &section.header;
            let word = |at: usize| {
                u32::from_be_bytes([
                    header.get(at).copied().unwrap_or(0),
                    header.get(at + 1).copied().unwrap_or(0),
                    header.get(at + 2).copied().unwrap_or(0),
                    header.get(at + 3).copied().unwrap_or(0),
                ])
            };
            println!("    header words: {:?}", (0..header.len() / 4).map(|i| word(i * 4)).collect::<Vec<_>>());
            let entry = word(0);
            let count = word(4);
            println!(
                "\n  {} entry {entry} bytes, {count} entries, payload {}",
                std::str::from_utf8(tag).unwrap_or("?"),
                section.payload.len()
            );
            let stride = entry.max(1) as usize;
            if stride == 3 {
                let mut maxes = [0_u8; 3];
                let mut sums = [0_u64; 3];
                let mut n = 0_u64;
                for chunk in section.payload.chunks_exact(3) {
                    for b in 0..3 {
                        maxes[b] = maxes[b].max(chunk[b]);
                        sums[b] += u64::from(chunk[b]);
                    }
                    n += 1;
                }
                println!(
                    "    band max {maxes:?}  mean {:?}",
                    sums.map(|s| s.checked_div(n).unwrap_or(0))
                );
                // Stacked or overlaid? If the three are meant to sit on top of
                // one another the sum should fill the scale; if they overlap
                // from a baseline the largest alone should.
                let mut sum_max = 0_u32;
                let mut over = 0_u32;
                for chunk in section.payload.chunks_exact(3) {
                    let total = u32::from(chunk[0]) + u32::from(chunk[1]) + u32::from(chunk[2]);
                    sum_max = sum_max.max(total);
                    if total > 63 { over += 1; }
                }
                println!("    stacked max {sum_max}  columns over 63: {over} of {n}");
            }
            // A loud stretch, so the channels are not all near zero.
            let start = section.payload.len() / 3 / stride * stride;
            for column in 0..8 {
                let at = start + column * stride;
                let Some(bytes) = section.payload.get(at..at + stride) else { break };
                println!("    [{column}] {bytes:?}");
            }
        }
    }
}
