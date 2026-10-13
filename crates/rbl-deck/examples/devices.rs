//! Which outputs the audio could go to. READ-ONLY.
//!
//! `cargo run -p rbl-deck --example devices`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

fn main() {
    let default = rbl_deck::default_output_device();
    let devices = rbl_deck::output_devices();
    println!("{} output devices", devices.len());
    for device in &devices {
        let mark = if default.as_ref().is_some_and(|d| d.id == device.id) { "*" } else { " " };
        println!("{mark} {:<44} {}", device.name, device.id);
    }
    match default {
        Some(device) => println!("\ndefault: {} ({})", device.name, device.id),
        None => println!("\nno default output device"),
    }
}
