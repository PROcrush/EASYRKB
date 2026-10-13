//! Read-only waveform diagnostics: audio path and output directory.
#![allow(clippy::print_stdout)]
use std::{io::Write, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let source = args.get(1).ok_or("audio path required")?;
    let out = Path::new(args.get(2).ok_or("output directory required")?);
    std::fs::create_dir_all(out)?;
    let audio = rbl_audio::decode_mono(Path::new(source), None)?;
    let mut file = std::io::BufWriter::new(std::fs::File::create(out.join("mono.f32"))?);
    for sample in &audio.samples { file.write_all(&sample.to_le_bytes())?; }
    file.flush()?;
    let wave = rbl_analysis::waveform::compute(&audio.samples, audio.sample_rate);
    let columns: Vec<_> = wave.columns.iter().map(|c| rbl_anlz::BandColumn { low:c.low, mid:c.mid, high:c.high, peak:c.peak }).collect();
    std::fs::write(out.join("generated-pwv6.bin"), wave.overview.iter().flatten().copied().collect::<Vec<_>>())?;
    std::fs::write(out.join("generated-pwv7.bin"), rbl_anlz::encode::pwv7(&columns))?;
    std::fs::write(out.join("sample-rate.txt"), audio.sample_rate.to_string())?;
    println!("{} samples at {} Hz, {} columns", audio.samples.len(), audio.sample_rate, wave.columns.len());
    Ok(())
}
