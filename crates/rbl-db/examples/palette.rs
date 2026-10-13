//! Is the hot-cue colour palette in one of rekordbox's own databases?
//! READ-ONLY, and against the application bundle rather than the library.
//!
//! `ColorTableIndex` is `[UNKNOWN]`: what RGB an index past the default means
//! has not been found, and the note in the TODO says the non-invasive sources
//! were exhausted. This tries the two that were not: the `product.db` and the
//! empty `master.db` template inside rekordbox.app, both of which are
//! SQLCipher and neither of which anyone had opened.
//!
//! `cargo run -p rbl-db --example palette`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

const RESOURCES: &str = "/Applications/rekordbox 7/rekordbox.app/Contents/Resources";

fn main() {
    // The installed library's passphrase, which is what the app uses for its
    // own databases too if anything does.
    let passphrase = match rbl_db::detect() {
        Ok(location) => location.passphrase,
        Err(e) => {
            println!("cannot read the installed library's key: {e}");
            return;
        }
    };

    for name in ["product.db", "master.db"] {
        let path = Path::new(RESOURCES).join(name);
        if !path.exists() {
            println!("{name}: not there");
            continue;
        }
        println!("\n== {name}");
        match open(&path, &passphrase) {
            Ok(conn) => list(&conn),
            Err(e) => println!("  will not open: {e}"),
        }
    }
}

fn open(path: &Path, passphrase: &str) -> rusqlite::Result<rusqlite::Connection> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.pragma_update(None, "cipher", "sqlcipher")?;
    conn.pragma_update(None, "legacy", 4)?;
    conn.pragma_update(None, "key", passphrase)?;
    // The first real read is what proves the key.
    conn.query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0))?;
    Ok(conn)
}

fn list(conn: &rusqlite::Connection) {
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .expect("read the schema");
    let names: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .expect("read the schema")
        .collect::<Result<_, _>>()
        .expect("read the schema");
    println!("  {} tables", names.len());
    for name in &names {
        let rows: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM `{name}`"), [], |r| r.get(0))
            .unwrap_or(-1);
        let interesting = name.to_lowercase().contains("color") || name.to_lowercase().contains("cue");
        println!("  {}{name}: {rows} rows", if interesting { "* " } else { "  " });
    }
}
