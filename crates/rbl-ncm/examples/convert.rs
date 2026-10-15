#![allow(
    clippy::print_stdout,
    reason = "This command-line example prints the converted file's path."
)]

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("Usage: convert <source.ncm> <output-directory>".into());
    }
    let result = rbl_ncm::convert_file(
        std::path::Path::new(&args[0]),
        std::path::Path::new(&args[1]),
    )?;
    println!(
        "{}\t{}\t{}",
        result.format,
        result.bytes,
        result.path.display()
    );
    Ok(())
}
