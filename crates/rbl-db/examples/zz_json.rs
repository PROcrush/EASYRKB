//! Throwaway: one query's rows as JSON objects, full values.
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]
fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().unwrap(); let sql = a.next().unwrap();
    let conn = rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    conn.pragma_update(None, "cipher", "sqlcipher").unwrap();
    conn.pragma_update(None, "legacy", 4).unwrap();
    conn.pragma_update(None, "key", rbl_db::key::derive_password("FJ9s0iA+hiPZgURNVQNg+Aj/UQ41IlitwloFsPnU3sISVHn5EVNQwthYGuUdAryEcCzJZHnZ5Q7JoupTY9FDRw==").unwrap()).unwrap();
    let mut st = conn.prepare(&sql).unwrap();
    let names: Vec<String> = st.column_names().iter().map(|s| s.to_string()).collect();
    let mut rows = st.query([]).unwrap();
    while let Some(r) = rows.next().unwrap() {
        let mut m = serde_json::Map::new();
        for (i, n) in names.iter().enumerate() {
            let v = match r.get_ref(i).unwrap() {
                rusqlite::types::ValueRef::Null => serde_json::Value::Null,
                rusqlite::types::ValueRef::Integer(x) => x.into(),
                rusqlite::types::ValueRef::Real(x) => x.into(),
                rusqlite::types::ValueRef::Text(t) => String::from_utf8_lossy(t).into_owned().into(),
                rusqlite::types::ValueRef::Blob(b) => format!("<{}B>", b.len()).into(),
            };
            m.insert(n.clone(), v);
        }
        println!("{}", serde_json::Value::Object(m));
    }
}
