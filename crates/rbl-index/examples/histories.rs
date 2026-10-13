//! READ-ONLY: the history tree as the index holds it, with member counts.
//! `cargo run --release -p rbl-index --example histories`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

fn main() {
    let db = rbl_db::Library::open_installed_read_only().expect("open");
    let (library, _) = rbl_index::load(&db).expect("index");
    let h = library.histories();
    println!("{} history nodes", h.len());
    for i in 0..h.len() {
        let parent = h.parent.get(i).copied().unwrap_or(rbl_index::NO_ID);
        let depth = {
            let mut d = 0; let mut p = parent;
            while p != rbl_index::NO_ID && (p as usize) < h.len() && d < 8 { d += 1; p = h.parent[p as usize]; }
            d
        };
        if i < 40 || !h.members[i].is_empty() && i % 20 == 0 {
            println!("{}{} [{}] folder={} members={} seq={}", "  ".repeat(depth), h.name(i), h.ids[i], h.is_folder(i), h.members[i].len(), h.seq[i]);
        }
    }
    // The newest session by id from the table above.
    if let Some(i) = h.ids.iter().position(|&id| id == 1_662_826_848) {
        println!("\nLINK HISTORY 2026-09-04: {} members; first rows {:?}", h.members[i].len(), &h.members[i][..5.min(h.members[i].len())]);
        for &row in h.members[i].iter().take(5) {
            println!("  {}", library.title.get(row as usize));
        }
    }
    let with_members = (0..h.len()).filter(|&i| !h.members[i].is_empty()).count();
    println!("\nsessions with members: {with_members}");
}
