//! Records what rekordbox changes in its own database.
//!
//! ```text
//!   rbl-difftool snapshot <out.json>       # read-only; safe any time
//!   rbl-difftool diff <before.json> <after.json> [--json]
//!   rbl-difftool record <name>             # guided: snapshot, prompt, snapshot, diff
//! ```
//!
//! The intended protocol, which is the only way to learn what a field means
//! without guessing:
//!
//! 1. Quit rekordbox.
//! 2. `snapshot before.json`
//! 3. Start rekordbox, perform exactly ONE action, quit it.
//! 4. `snapshot after.json`
//! 5. `diff before.json after.json`
#![allow(clippy::print_stdout, clippy::print_stderr, clippy::cast_precision_loss)]

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map_or("help", String::as_str);

    match command {
        "snapshot" => match args.get(1) {
            Some(out) => run_snapshot(Path::new(out)),
            None => usage("snapshot needs an output path"),
        },
        "diff" => match (args.get(1), args.get(2)) {
            (Some(a), Some(b)) => {
                run_diff(Path::new(a), Path::new(b), args.iter().any(|x| x == "--json"))
            }
            _ => usage("diff needs two snapshot paths"),
        },
        "inspect" => match (args.get(1), args.get(2)) {
            (Some(table), Some(key)) => run_inspect(table, key),
            _ => usage("inspect needs a table and a row id"),
        },
        "record" => match args.get(1) {
            Some(name) => run_record(name),
            None => usage("record needs a name, e.g. `create-playlist`"),
        },
        _ => usage(""),
    }
}

fn usage(message: &str) -> ExitCode {
    if !message.is_empty() {
        eprintln!("error: {message}\n");
    }
    eprintln!(
        "rbl-difftool — records what rekordbox changes in master.db\n\n\
         USAGE\n  \
           rbl-difftool snapshot <out.json>\n  \
           rbl-difftool diff <before.json> <after.json> [--json]\n  \
           rbl-difftool inspect <table> <id>\n  \
           rbl-difftool record <name>\n\n\
         All database access is READ-ONLY.\n\n\
         PROTOCOL\n  \
           1. Quit rekordbox\n  \
           2. snapshot before.json\n  \
           3. Start rekordbox, do ONE thing, quit it\n  \
           4. snapshot after.json\n  \
           5. diff before.json after.json"
    );
    ExitCode::FAILURE
}

fn open_read_only() -> Option<rbl_db::Library> {
    match rbl_db::Library::open_installed_read_only() {
        Ok(db) => Some(db),
        Err(e) => {
            eprintln!("could not open the library: {e}");
            None
        }
    }
}

fn run_snapshot(out: &Path) -> ExitCode {
    let Some(db) = open_read_only() else { return ExitCode::FAILURE };
    if rbl_db::is_rekordbox_running() {
        eprintln!(
            "warning: rekordbox is running. A snapshot taken now may catch it mid-write; \
             quit it first for a clean recording."
        );
    }
    match rbl_difftool::snapshot(db.connection()) {
        Ok(snapshot) => {
            let tables = snapshot.tables.len();
            let full: usize = snapshot.tables.values().filter(|t| t.full).count();
            let full_rows: usize = snapshot.tables.values().filter(|t| t.full).map(|t| t.rows.len()).sum();
            let digest_rows: usize = snapshot.tables.values().map(|t| t.digests.len()).sum();
            match rbl_difftool::write_snapshot(out, &snapshot) {
                Ok(()) => {
                    let size = std::fs::metadata(out).map_or(0, |m| m.len());
                    println!(
                        "wrote {} ({:.1} MB)\n  {tables} tables: {full} stored in full ({full_rows} rows), \
                         {} by digest ({digest_rows} rows)",
                        out.display(),
                        size as f64 / 1_048_576.0,
                        tables - full,
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("could not write {}: {e}", out.display());
                    ExitCode::FAILURE
                }
            }
        }
        Err(e) => {
            eprintln!("snapshot failed: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run_diff(before: &Path, after: &Path, as_json: bool) -> ExitCode {
    let (Ok(a), Ok(b)) = (
        rbl_difftool::read_snapshot(before),
        rbl_difftool::read_snapshot(after),
    ) else {
        eprintln!("could not read both snapshots");
        return ExitCode::FAILURE;
    };
    let d = rbl_difftool::diff(&a, &b);
    if as_json {
        match serde_json::to_string_pretty(&d) {
            Ok(text) => println!("{text}"),
            Err(e) => {
                eprintln!("could not render the diff: {e}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        print!("{}", d.summarise());
    }
    ExitCode::SUCCESS
}

/// Renders a value the way a person reads it. `ValueRef`'s own Debug prints
/// text as a byte array, which is unusable in a recording.
fn render_value(value: rusqlite::types::ValueRef<'_>) -> String {
    use rusqlite::types::ValueRef;
    match value {
        ValueRef::Null => "NULL".to_owned(),
        ValueRef::Integer(v) => v.to_string(),
        ValueRef::Real(v) => v.to_string(),
        ValueRef::Text(t) => format!("{:?}", String::from_utf8_lossy(t)),
        ValueRef::Blob(b) => format!("<blob {} bytes>", b.len()),
    }
}

/// Prints one row from the live database, for keys a digest-only diff named.
fn run_inspect(table: &str, key: &str) -> ExitCode {
    let Some(db) = open_read_only() else { return ExitCode::FAILURE };
    // The table name comes from a diff we produced, but validate anyway rather
    // than interpolating an arbitrary string into SQL.
    if !table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        eprintln!("refusing to inspect a table name with unexpected characters: {table:?}");
        return ExitCode::FAILURE;
    }
    let conn = db.connection();
    // Most djmd tables key on `ID`, but not all (djmdProperty uses DBID), and
    // snapshots fall back to the row index when there is no ID at all.
    let key_column = ["ID", "DBID", "rowid"]
        .into_iter()
        .find(|candidate| {
            conn.prepare(&format!("SELECT {candidate} FROM {table} LIMIT 1")).is_ok()
        });
    let Some(key_column) = key_column else {
        eprintln!("no such table, or no usable key column: {table}");
        return ExitCode::FAILURE;
    };
    let sql = format!("SELECT * FROM {table} WHERE {key_column} = ?1");
    let Ok(mut stmt) = conn.prepare(&sql) else {
        eprintln!("could not query {table}");
        return ExitCode::FAILURE;
    };
    let names: Vec<String> = stmt.column_names().iter().map(|s| (*s).to_owned()).collect();
    let printed = stmt.query_row([key], |row| {
        for (i, name) in names.iter().enumerate() {
            let value = row.get_ref(i).map(render_value).unwrap_or_default();
            println!("  {name:<24} {value}");
        }
        Ok(())
    });
    match printed {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("could not read {table} row {key}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The eight recordings the plan calls for, and the one action each needs.
///
/// Named here rather than left to memory: a recording of two actions at once
/// is worthless, because the diff cannot say which change came from which.
const ACTIONS: [(&str, &str); 8] = [
    ("create-playlist", "Create ONE new playlist. Do not put anything in it."),
    ("add-track", "Drag ONE track into that playlist."),
    ("hot-cue", "Set hot cue A on ONE track."),
    ("loop", "Save a 4-beat loop on ONE track."),
    ("rating", "Give ONE track a 4-star rating."),
    ("analyze", "Analyse ONE track that has never been analysed."),
    ("import", "Import ONE audio file from outside the library."),
    ("delete", "Delete ONE playlist you created for this."),
];

/// Runs the whole record-diff-record cycle, waiting for rekordbox in between.
///
/// The protocol was five commands typed in the right order with a quit either
/// side, which is easy to get wrong in a way that silently produces a useless
/// recording. This is one command that waits for the right moments itself.
/// Everything it does to the database is a read.
fn run_record(name: &str) -> ExitCode {
    let action = ACTIONS.iter().find(|(n, _)| *n == name).map(|(_, a)| *a);
    let Some(action) = action else {
        eprintln!("unknown recording {name:?}. One of:");
        for (n, a) in ACTIONS {
            eprintln!("  {n:<16} {a}");
        }
        return ExitCode::FAILURE;
    };

    let dir = Path::new("recordings");
    if let Err(e) = std::fs::create_dir_all(dir) {
        eprintln!("could not create {}: {e}", dir.display());
        return ExitCode::FAILURE;
    }
    let before = dir.join(format!("{name}.before.json.gz"));
    let after = dir.join(format!("{name}.after.json.gz"));

    println!("Recording \"{name}\".\n");
    if !wait_for_rekordbox(false, "Quit rekordbox to start the recording.") {
        return ExitCode::FAILURE;
    }
    println!("\nTaking the \"before\" snapshot…");
    if run_snapshot(&before) != ExitCode::SUCCESS {
        return ExitCode::FAILURE;
    }

    println!("\n  ACTION: {action}\n         Do that and nothing else.\n");
    if !wait_for_rekordbox(true, "Start rekordbox when you are ready.") {
        return ExitCode::FAILURE;
    }
    if !wait_for_rekordbox(false, "Quit rekordbox once the action is done.") {
        return ExitCode::FAILURE;
    }
    // rekordbox flushes on exit; a snapshot taken the instant the process
    // disappears can miss the last write.
    std::thread::sleep(std::time::Duration::from_secs(3));

    println!("\nTaking the \"after\" snapshot…");
    if run_snapshot(&after) != ExitCode::SUCCESS {
        return ExitCode::FAILURE;
    }

    let (Ok(a), Ok(b)) = (
        rbl_difftool::read_snapshot(&before),
        rbl_difftool::read_snapshot(&after),
    ) else {
        eprintln!("could not read back both snapshots");
        return ExitCode::FAILURE;
    };
    let summary = rbl_difftool::diff(&a, &b).summarise();
    let out = dir.join(format!("{name}.diff.txt"));
    if let Err(e) = std::fs::write(&out, &summary) {
        eprintln!("could not write {}: {e}", out.display());
        return ExitCode::FAILURE;
    }
    println!("\n{summary}");
    println!("kept as {}", out.display());
    ExitCode::SUCCESS
}

/// Blocks until rekordbox is running (or not), printing the prompt once.
///
/// Polls at 2 Hz, which is invisible next to a person switching applications
/// and costs nothing measurable.
fn wait_for_rekordbox(want_running: bool, prompt: &str) -> bool {
    if rbl_db::is_rekordbox_running() == want_running {
        return true;
    }
    println!("{prompt}  (waiting)");
    let started = std::time::Instant::now();
    while rbl_db::is_rekordbox_running() != want_running {
        // Ten minutes is long enough for any single action and short enough
        // that a forgotten terminal does not poll all night.
        if started.elapsed() > std::time::Duration::from_secs(600) {
            eprintln!("gave up waiting after ten minutes; nothing was written");
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    true
}
