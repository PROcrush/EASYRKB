//! Throwaway: runs SELECTs against a copied master.db with rekordbox's key.
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]
fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().unwrap();
    let conn = rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    conn.pragma_update(None, "cipher", "sqlcipher").unwrap();
    conn.pragma_update(None, "legacy", 4).unwrap();
    let key = rbl_db::key::derive_password("FJ9s0iA+hiPZgURNVQNg+Aj/UQ41IlitwloFsPnU3sISVHn5EVNQwthYGuUdAryEcCzJZHnZ5Q7JoupTY9FDRw==").unwrap();
    conn.pragma_update(None, "key", &key).unwrap();
    for sql in a {
        println!("== {sql}");
        let mut st = conn.prepare(&sql).unwrap();
        let n = st.column_count();
        let mut rows = st.query([]).unwrap();
        while let Some(r) = rows.next().unwrap() {
            let v: Vec<String> = (0..n).map(|i| match r.get_ref(i).unwrap() {
                rusqlite::types::ValueRef::Null => "NULL".into(),
                rusqlite::types::ValueRef::Integer(x) => x.to_string(),
                rusqlite::types::ValueRef::Real(x) => x.to_string(),
                rusqlite::types::ValueRef::Text(t) => { let s = String::from_utf8_lossy(t); s.chars().take(90).collect() }
                rusqlite::types::ValueRef::Blob(b) => format!("<{}B>", b.len()),
            }).collect();
            println!("{}", v.join(" | "));
        }
    }
}
