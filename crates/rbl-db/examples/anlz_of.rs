//! READ-ONLY: the analysis files of one track by `djmdContent.ID`, and the
//! tags inside each, so a captured link-export blob can be compared with the
//! file it came from.
//! `cargo run -q -p rbl-db --example anlz_of -- <content id>`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

fn main() {
    let id = std::env::args().nth(1).expect("content id");
    let db = rbl_db::Library::open_installed_read_only().expect("open");
    let conn = db.connection();
    let (path, folder, size): (String, String, i64) = conn
        .query_row(
            "SELECT AnalysisDataPath, FolderPath, FileSize FROM djmdContent WHERE ID = ?1",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, Option<i64>>(2)?.unwrap_or(0))),
        )
        .unwrap();
    println!("file: {folder} ({size} bytes)");
    let share = db.location().share_root.clone();
    let base = share.join(path.trim_start_matches('/'));
    println!("analysis: {}", base.display());
    for ext in ["DAT", "EXT", "2EX"] {
        let file = base.with_extension(ext);
        let Ok(bytes) = std::fs::read(&file) else {
            println!("  {ext}: missing");
            continue;
        };
        println!("  {ext}: {} bytes", bytes.len());
        // Walk the tags: after the 28-byte PMAI header, each tag is fourcc, header len, tag len.
        let mut at = 28;
        while at + 12 <= bytes.len() {
            let fourcc = String::from_utf8_lossy(&bytes[at..at + 4]).to_string();
            let len = u32::from_be_bytes(bytes[at + 8..at + 12].try_into().unwrap()) as usize;
            println!("    {fourcc} at {at} len {len}");
            std::fs::write(format!("/tmp/blobs/file-{ext}-{fourcc}.bin"), &bytes[at..(at + len).min(bytes.len())]).unwrap();
            if len == 0 { break; }
            at += len;
        }
    }
}
