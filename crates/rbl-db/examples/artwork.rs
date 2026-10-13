//! What `djmdContent.ImagePath` holds, and whether those files exist.
//! READ-ONLY.
//!
//! `cargo run -p rbl-db --example artwork`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

fn main() {
    let db = match rbl_db::Library::open_installed_read_only() {
        Ok(db) => db,
        Err(e) => {
            println!("cannot open library: {e}");
            return;
        }
    };
    let conn = db.connection();
    let share = db.location().share_root.clone();
    println!("share root: {}\n", share.display());

    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM djmdContent WHERE rb_local_deleted = 0 \
             AND ImagePath IS NOT NULL AND ImagePath != ''",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let all: i64 = conn
        .query_row("SELECT COUNT(*) FROM djmdContent WHERE rb_local_deleted = 0", [], |r| r.get(0))
        .unwrap_or(0);
    println!("tracks with an ImagePath: {total} of {all}\n");

    let mut stmt = conn
        .prepare(
            "SELECT ImagePath FROM djmdContent WHERE rb_local_deleted = 0 \
             AND ImagePath IS NOT NULL AND ImagePath != '' LIMIT 6",
        )
        .unwrap();
    let paths: Vec<String> =
        stmt.query_map([], |r| r.get(0)).unwrap().filter_map(Result::ok).collect();
    for path in &paths {
        // Paths are stored share-relative with a leading separator.
        let full = share.join(path.trim_start_matches(['/', '\\']));
        println!("  {path}\n    -> {} {}", full.display(), if full.exists() { "EXISTS" } else { "missing" });
    }
}
