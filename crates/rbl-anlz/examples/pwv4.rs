//! READ-ONLY: how the six bytes of a `PWV4` column relate to the three of
//! `PWV6` for the same column, over a few real tracks. Both are 1200 columns.
//! `cargo run --release -p rbl-anlz --example pwv4`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used, clippy::cast_precision_loss)]

fn pearson(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len() as f64;
    let ma = a.iter().sum::<f64>() / n;
    let mb = b.iter().sum::<f64>() / n;
    let cov: f64 = a.iter().zip(b).map(|(x, y)| (x - ma) * (y - mb)).sum();
    let va: f64 = a.iter().map(|x| (x - ma).powi(2)).sum();
    let vb: f64 = b.iter().map(|y| (y - mb).powi(2)).sum();
    cov / (va.sqrt() * vb.sqrt()).max(1e-9)
}

fn main() {
    let db = rbl_db::Library::open_installed_read_only().expect("open");
    let share = db.location().share_root.clone();
    let (library, _) = rbl_index::load(&db).expect("index");
    let mut seen = 0;
    for i in (0..library.len()).step_by(997) {
        if seen >= 6 { break; }
        let dat = rbl_anlz::resolve(&share, library.analysis_path.get(i));
        let (Ok(ext), Ok(ex2)) = (
            rbl_anlz::Anlz::read(&rbl_anlz::sibling(&dat, "EXT")),
            rbl_anlz::Anlz::read(&rbl_anlz::sibling(&dat, "2EX")),
        ) else { continue };
        let (Some((_, p4)), Some((_, p6))) = (ext.waveform(b"PWV4"), ex2.waveform(b"PWV6")) else { continue };
        if p4.len() != 7200 || p6.len() != 3600 { continue; }
        seen += 1;
        println!("\n{}", library.title.get(i));
        let cols4: Vec<Vec<f64>> = (0..6).map(|b| p4.chunks_exact(6).map(|c| f64::from(c[b])).collect()).collect();
        let cols6: Vec<Vec<f64>> = (0..3).map(|b| p6.chunks_exact(3).map(|c| f64::from(c[b])).collect()).collect();
        for (b, col) in cols4.iter().enumerate() {
            let max = col.iter().cloned().fold(0.0, f64::max);
            let mean = col.iter().sum::<f64>() / col.len() as f64;
            let r: Vec<String> = cols6.iter().map(|c6| format!("{:+.2}", pearson(col, c6))).collect();
            println!("  byte {b}: max {max:>3} mean {mean:>6.1}   r vs PWV6[mid,high,low] = {}", r.join(" "));
        }
        // And the sum of bytes 3..5 against PWV6's sum, in case they are bands.
        let sum4: Vec<f64> = p4.chunks_exact(6).map(|c| f64::from(c[3]) + f64::from(c[4]) + f64::from(c[5])).collect();
        let sum6: Vec<f64> = p6.chunks_exact(3).map(|c| f64::from(c[0]) + f64::from(c[1]) + f64::from(c[2])).collect();
        println!("  bytes3+4+5 vs PWV6 sum r = {:+.2}", pearson(&sum4, &sum6));

        // PWV5 against PWV7: both 150 columns a second, the same count.
        let (Some((_, p5)), Some((_, p7))) = (ext.waveform(b"PWV5"), ex2.waveform(b"PWV7")) else { continue };
        if p5.len() / 2 != p7.len() / 3 { println!("  PWV5/PWV7 counts differ"); continue; }
        let words: Vec<u16> = p5.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
        let cols7: Vec<Vec<f64>> = (0..3).map(|b| p7.chunks_exact(3).map(|c| f64::from(c[b])).collect()).collect();
        for (name, shift, mask) in [("b15-13", 13, 7), ("b12-10", 10, 7), ("b9-7", 7, 7), ("b6-2", 2, 31), ("b4-0", 0, 31), ("b7-3", 3, 31), ("b1-0", 0, 3)] {
            let field: Vec<f64> = words.iter().map(|w| f64::from((w >> shift) & mask)).collect();
            let max = field.iter().cloned().fold(0.0, f64::max);
            let r: Vec<String> = cols7.iter().map(|c7| format!("{:+.2}", pearson(&field, c7))).collect();
            println!("  PWV5 {name}: max {max:>2}  r vs PWV7[mid,high,low] = {}", r.join(" "));
        }
    }
}

#[allow(dead_code)]
fn unused() {}
