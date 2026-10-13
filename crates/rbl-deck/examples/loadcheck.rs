//! Which of the real library's tracks the deck cannot load, and why. READ-ONLY.
//!
//! Decodes a sample of the installed library through a null sink, so it asks
//! whether a file opens rather than whether it is audible, and prints where the
//! library's audio lives and which of those volumes are mounted. A track whose
//! drive is unplugged is the usual reason the player refuses one.
//!
//! `cargo run --release -p rbl-deck --example loadcheck -- [count]`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::{mpsc, Arc};

/// How long to wait for one file's load event before calling it a failure.
const PATIENCE: std::time::Duration = std::time::Duration::from_secs(20);

fn main() {
    let want: usize = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(400);
    let db = match rbl_db::Library::open_installed_read_only() {
        Ok(db) => db,
        Err(e) => {
            println!("cannot open library: {e}");
            return;
        }
    };
    let (library, _) = rbl_index::load(&db).expect("index");

    let (tx, rx) = mpsc::channel::<rbl_deck::DeckEvent>();
    let events: rbl_deck::EventSink = Arc::new(move |e| {
        let _ = tx.send(e);
    });
    let engine = rbl_deck::Engine::with_sink(
        |render| Ok(Arc::new(rbl_deck::NullSink::new(44_100, render)) as Arc<dyn rbl_deck::Sink>),
        &events,
    )
    .expect("engine");

    let total = library.len();
    let step = (total / want.max(1)).max(1);
    let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
    let mut examples: Vec<String> = Vec::new();
    let (mut ok, mut missing, mut tried) = (0_usize, 0_usize, 0_usize);

    for row in (0..total).step_by(step).take(want) {
        let path = std::path::PathBuf::from(library.folder_path.get(row));
        if !path.exists() {
            missing += 1;
            if examples.len() < 8 {
                examples.push(format!("no file: {}", path.display()));
            }
            continue;
        }
        tried += 1;
        engine.load(rbl_deck::Deck::A, &path);
        match rx.recv_timeout(PATIENCE) {
            Ok(rbl_deck::DeckEvent::Loaded { .. }) => ok += 1,
            Ok(rbl_deck::DeckEvent::Error { message, .. }) => {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                *reasons.entry(format!("[{ext}] {message}")).or_default() += 1;
                if examples.len() < 8 {
                    examples.push(path.display().to_string());
                }
            }
            Err(e) => *reasons.entry(format!("no event: {e}")).or_default() += 1,
        }
    }

    println!("{tried} tracks opened, {ok} loaded, {missing} had no file on disk");
    for (reason, count) in &reasons {
        println!("  {count:5}  {reason}");
    }
    for path in &examples {
        println!("  e.g. {path}");
    }

    println!("\nwhere the whole library's audio lives");
    let mut roots: BTreeMap<String, usize> = BTreeMap::new();
    for row in 0..total {
        let root = library.folder_path.get(row).split('/').take(3).collect::<Vec<_>>().join("/");
        *roots.entry(root).or_default() += 1;
    }
    for (root, count) in &roots {
        let mounted = std::path::Path::new(root).exists();
        println!("  {count:6}  {root}  {}", if mounted { "mounted" } else { "NOT MOUNTED" });
    }
}
