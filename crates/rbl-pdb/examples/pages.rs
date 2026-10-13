//! Prints every page of an export.pdb with its header fields and the raw
//! bytes of every row, to compare a file rekordbox wrote with one we wrote.
//! READ-ONLY.
//!
//! `cargo run -p rbl-pdb --example pages -- <export.pdb> [table-type ...]`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

fn u2(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}
fn u4(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}
fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        println!("usage: pages <export.pdb> [table-type ...]");
        return;
    };
    let only: Vec<u32> = args.filter_map(|a| a.parse().ok()).collect();
    let bytes = std::fs::read(&path).expect("read");
    let page_size = u4(&bytes, 4) as usize;
    let num_tables = u4(&bytes, 8) as usize;
    println!("file {} bytes, {} pages of {page_size}", bytes.len(), bytes.len() / page_size);
    println!("header: {}", hex(&bytes[..0x1c]));
    for i in 0..num_tables {
        let at = 28 + i * 16;
        println!(
            "  table type={:<2} empty_candidate={:<4} first={:<4} last={:<4}",
            u4(&bytes, at), u4(&bytes, at + 4), u4(&bytes, at + 8), u4(&bytes, at + 12)
        );
    }
    let tail = 28 + num_tables * 16;
    let rest = &bytes[tail..page_size];
    if rest.iter().any(|&b| b != 0) {
        println!("  header page tail (non-zero): {}", hex(&rest[..rest.len().min(64)]));
    }
    for index in 1..bytes.len() / page_size {
        let p = &bytes[index * page_size..(index + 1) * page_size];
        let page_type = u4(p, 0x08);
        if !only.is_empty() && !only.contains(&page_type) {
            continue;
        }
        let num_rows_small = p[0x18] as u32;
        let num_rows_large = u2(p, 0x22) as u32;
        println!(
            "page {index:<4} type={page_type:<2} idx={} next={} seq={} u1={} rows_small={} u2={:02x} flags={:02x} free={} used={} u3={} rows_large={} u4={} u5={}",
            u4(p, 0x04), u4(p, 0x0c), u4(p, 0x10), u4(p, 0x14), num_rows_small, p[0x19], p[0x1b],
            u2(p, 0x1c), u2(p, 0x1e), u2(p, 0x20), num_rows_large, u2(p, 0x24), u2(p, 0x26)
        );
        println!("  header: {}", hex(&p[..0x28]));
        let is_data = p[0x1b] & 0x40 == 0;
        let num_rows = if num_rows_large > num_rows_small && num_rows_large != 0x1fff { num_rows_large } else { num_rows_small };
        if !is_data || num_rows == 0 {
            let body = &p[0x28..];
            if body.iter().any(|&b| b != 0) {
                println!("  body (non-zero): {}", hex(&body[..body.len().min(96)]));
            }
            continue;
        }
        let groups = (num_rows - 1) / 16 + 1;
        let mut offsets: Vec<(u32, u16, bool)> = Vec::new();
        for group in 0..groups {
            let base = page_size - group as usize * 0x24;
            let present = u2(p, base - 4);
            let in_group = if group < groups - 1 { 16 } else { (num_rows - 1) % 16 + 1 };
            for row in 0..in_group {
                let ofs = u2(p, base - 6 - 2 * row as usize);
                offsets.push((group * 16 + row, ofs, present & (1 << row) != 0));
            }
            println!("  group {group}: present={present:04x} raw={}", hex(&p[base - 0x24..base]));
        }
        let mut sorted: Vec<u16> = offsets.iter().map(|o| o.1).collect();
        sorted.sort_unstable();
        for (n, ofs, present) in &offsets {
            let start = 0x28 + *ofs as usize;
            let end = sorted.iter().find(|&&o| o > *ofs).map_or(u2(p, 0x1c) as usize + 0x28, |&o| 0x28 + o as usize);
            let end = end.min(page_size).max(start);
            println!("  row {n:<3} @{ofs:04x} {}len={} {}", if *present { "" } else { "DELETED " }, end - start, hex(&p[start..end]));
        }
    }
}
