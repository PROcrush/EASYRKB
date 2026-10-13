#![allow(
    clippy::pedantic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]
use rbl_pdb::{build::FileBuilder, PageType, Pdb};
fn fixture() -> Vec<u8> {
    let rows: Vec<_> = (0_u32..600)
        .map(|i| {
            [i + 1, i + 1, 1]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>()
        })
        .collect();
    let mut builder = FileBuilder::new(4096);
    builder.add_table(8, &rows);
    builder.finish()
}
#[test]
fn valid_multi_page_database_has_no_diagnostics() {
    let bytes = fixture();
    let pdb = Pdb::parse(&bytes).unwrap();
    assert!(pdb.diagnostics().is_empty(), "{:?}", pdb.diagnostics());
    assert_eq!(
        pdb.playlist_entries(pdb.table(PageType::PlaylistEntries).unwrap())
            .len(),
        600
    );
}
#[test]
fn page_cycle_is_bounded_and_reported_without_duplicate_records() {
    let mut bytes = fixture();
    let pdb = Pdb::parse(&bytes).unwrap();
    let table = pdb.table(PageType::PlaylistEntries).unwrap();
    let first = table.first_page;
    bytes[first as usize * 4096 + 12..first as usize * 4096 + 16]
        .copy_from_slice(&first.to_le_bytes());
    let pdb = Pdb::parse(&bytes).unwrap();
    assert!(pdb.diagnostics().iter().any(|i| i.contains("repeats")));
    assert!(
        pdb.playlist_entries(pdb.table(PageType::PlaylistEntries).unwrap())
            .len()
            <= 600
    );
}
#[test]
fn broken_chain_retains_valid_rows_and_reports_missing_tail() {
    let mut bytes = fixture();
    let beyond = (bytes.len() / 4096 + 10) as u32;
    bytes[40..44].copy_from_slice(&beyond.to_le_bytes());
    let pdb = Pdb::parse(&bytes).unwrap();
    assert!(pdb
        .diagnostics()
        .iter()
        .any(|i| i.contains("ends before") || i.contains("is missing")));
    assert_eq!(
        pdb.playlist_entries(pdb.table(PageType::PlaylistEntries).unwrap())
            .len(),
        600
    );
    bytes.pop();
    assert!(Pdb::parse(&bytes)
        .unwrap()
        .diagnostics()
        .iter()
        .any(|i| i.contains("truncated")));
}
