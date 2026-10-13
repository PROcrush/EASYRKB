//! What `list_devices` sees, and how long it takes to see it.
//!
//! Enumeration runs on the app's startup path, so anything slow here is a
//! window that says "Loading…" and never stops.
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used)]

use std::time::Instant;

fn main() {
    let started = Instant::now();
    let devices = rbl_devices::list();
    println!("list(): {} device(s) in {:?}", devices.len(), started.elapsed());
    for device in &devices {
        println!(
            "  {:<24} {:<28} removable={} {} free of {}",
            device.name,
            device.mount_point.display(),
            device.removable,
            device.free_bytes,
            device.total_bytes
        );
        let at = Instant::now();
        let found = rbl_devices::inspect(&device.mount_point);
        println!("    inspect(): {found:?} in {:?}", at.elapsed());
    }
    println!("total {:?}", started.elapsed());
}
