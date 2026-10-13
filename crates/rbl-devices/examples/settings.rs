//! Prints what the device tabs would show for a mounted stick. READ-ONLY.
//!
//! `cargo run -p rbl-devices --example settings -- /Volumes/<stick>`
#![allow(clippy::pedantic, clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

fn main() {
    let Some(mount) = std::env::args().nth(1) else {
        println!("usage: settings <mount point>");
        return;
    };
    let mount = std::path::Path::new(&mount);
    println!("export root: {}", rbl_devices::settings::export_root(mount).display());
    println!("inspect: {:?}", rbl_devices::inspect(mount));
    let settings = rbl_devices::settings::read(mount);
    println!("device library: {}  one library: {}", settings.has_device_library, settings.has_one_library);
    println!("DEVSETTING.DAT: {:?}", settings.dev);
    if let Some(library) = settings.library {
        println!("device name: {:?}  colours: {:?}", library.device_name, library.colors.iter().map(|c| c.name.as_str()).collect::<Vec<_>>());
    }
}
