//! Dumps the raw section framing of a real ANLZ file.
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

fn be32(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn main() {
    let path = std::env::args().nth(1).expect("path");
    let b = std::fs::read(&path).expect("read");
    println!("{path}  ({} bytes)", b.len());
    println!("  magic {:?}  len_header {}  len_file {}",
             String::from_utf8_lossy(&b[0..4]), be32(&b, 4), be32(&b, 8));
    print!("  header bytes:");
    for byte in b.iter().take(be32(&b, 4).min(64) as usize) { print!(" {byte:02x}"); }
    println!();

    let mut at = be32(&b, 4) as usize;
    while at + 12 <= b.len() {
        let tag = String::from_utf8_lossy(&b[at..at + 4]).to_string();
        let len_header = be32(&b, at + 4);
        let len_tag = be32(&b, at + 8);
        print!("  {tag}  len_header {len_header:>3}  len_tag {len_tag:>8}   body[0..16]:");
        let body_start = at + len_header as usize;
        for i in 0..16 {
            if body_start + i < b.len() && body_start + i < at + len_tag as usize {
                print!(" {:02x}", b[body_start + i]);
            }
        }
        println!();
        if len_tag < 12 { break; }
        at += len_tag as usize;
    }
}
