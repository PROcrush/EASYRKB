//! Prints the device settings a real `exportLibrary.db` carries, as the
//! device panel's tabs would show them. READ-ONLY.
//!
//! `cargo run -p rbl-onelibrary --example stick_settings -- <exportLibrary.db>`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use rbl_onelibrary::settings::StickSettings;

fn main() {
    let Some(path) = std::env::args().nth(1).map(PathBuf::from) else {
        println!("usage: stick_settings <exportLibrary.db>");
        return;
    };
    let settings = StickSettings::read(&path).expect("read");
    println!("device name: {:?}", settings.device_name);
    println!("backGroundColorType: {}", settings.background_color_type);
    for (label, slots) in [("categories", &settings.categories), ("sorts", &settings.sorts)] {
        let mut active: Vec<_> = slots.iter().filter(|s| s.visible).collect();
        active.sort_by_key(|s| s.seq);
        let mut inactive: Vec<_> = slots.iter().filter(|s| !s.visible).collect();
        inactive.sort_by(|a, b| a.name.cmp(&b.name));
        println!("{label} active:   {:?}", active.iter().map(|s| s.name.as_str()).collect::<Vec<_>>());
        println!("{label} inactive: {:?}", inactive.iter().map(|s| s.name.as_str()).collect::<Vec<_>>());
    }
    println!("sub column: {:?}", settings.sub_column);
    println!("colors: {:?}", settings.colors.iter().map(|c| c.name.as_str()).collect::<Vec<_>>());
}
