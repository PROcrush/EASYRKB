//! Every `djmdKey.ScaleName` the library holds, and how often it is used.
//! READ-ONLY.
//!
//! Key sync has to turn these strings into a pitch class and a mode, and the
//! plan says to parse the string rather than trust `Seq` — there are duplicate
//! rows per key and the sequence is not a circle index. This prints what there
//! actually is to parse.
//!
//! `cargo run -p rbl-db --example keys`
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
    let mut stmt = conn
        .prepare(
            "SELECT k.ID, k.ScaleName, k.Seq, COUNT(c.ID)
             FROM djmdKey k LEFT JOIN djmdContent c
               ON c.KeyID = k.ID AND c.rb_local_deleted = 0
             WHERE k.rb_local_deleted = 0
             GROUP BY k.ID ORDER BY COUNT(c.ID) DESC",
        )
        .unwrap();
    let mut rows = stmt.query([]).unwrap();
    let mut total = 0_i64;
    let mut unparsed = 0_i64;
    while let Some(r) = rows.next().unwrap() {
        let id: String = r.get(0).unwrap();
        let name: Option<String> = r.get(1).unwrap();
        let seq: Option<i64> = r.get(2).unwrap();
        let used: i64 = r.get(3).unwrap();
        total += used;
        let text = name.unwrap_or_default();
        // What the parser makes of it, printed beside the string it came from:
        // anything this cannot read is a track key sync would have to refuse.
        let read = rbl_core::musickey::parse(&text);
        if read.is_none() {
            unparsed += used;
        }
        println!(
            "{id:>10}  {text:<12} seq={:<5} {used:>5} tracks  ->  {}",
            seq.unwrap_or(-1),
            read.map_or_else(|| "UNREADABLE".to_owned(), |k| format!("{:>2} {:?} = {}", k.pitch, k.mode, k.name())),
        );
    }
    println!("{total} tracks carry a key, {unparsed} of them unreadable");
}
