//! READ-ONLY: how big each table the load reads is on disk.
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]
fn main() {
    let db = rbl_db::Library::open_installed_read_only().expect("open");
    let conn = db.connection();
    let ps: i64 = conn.query_row("PRAGMA page_size", [], |r| r.get::<_, String>(0)).unwrap().parse().unwrap();
    let pc: i64 = conn.query_row("PRAGMA page_count", [], |r| r.get(0)).unwrap();
    println!("page_size {ps} page_count {pc} ({} MB)", ps * pc / 1_048_576);
    match conn.prepare("SELECT name, SUM(pgsize)/1048576, COUNT(*) FROM dbstat GROUP BY name ORDER BY 2 DESC LIMIT 25") {
        Ok(mut s) => {
            let mut rows = s.query([]).unwrap();
            while let Some(r) = rows.next().unwrap() {
                let (n, mb, pages): (String, i64, i64) = (r.get(0).unwrap(), r.get(1).unwrap(), r.get(2).unwrap());
                println!("{mb:>6} MB {pages:>8} pages  {n}");
            }
        }
        Err(e) => println!("no dbstat: {e}"),
    };
}
