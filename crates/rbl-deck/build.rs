//! Compiles the vendored Rubber Band Library.
//!
//! Upstream ships `single/RubberBandSingle.cpp`, one translation unit that
//! `#include`s the rest of the sources and needs no external library: the
//! built-in FFT and resampler everywhere, Apple's vDSP on macOS, which is why
//! the Accelerate framework is linked there. Vendored rather than found on the
//! system so that a Windows runner, a macOS runner and this machine all build
//! the same code.

#[cfg(not(feature = "rubberband"))]
fn main() {
    println!("cargo:rerun-if-changed=vendor/rubberband");
}

#[cfg(feature = "rubberband")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=vendor/rubberband");
    build()
}

/// Only compiled with the feature, because `cc` is only a dependency with it:
/// a build without Rubber Band must not need a C++ compiler at all.
#[cfg(feature = "rubberband")]
fn build() -> Result<(), Box<dyn std::error::Error>> {
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .include("vendor/rubberband")
        .file("vendor/rubberband/single/RubberBandSingle.cpp")
        // Upstream's own warnings are upstream's business, and there are a
        // lot of them; ours are still on for our code.
        .warnings(false);

    match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("windows") => {
            // `windows.h` defines `min` and `max` as macros, and Rubber Band
            // calls `std::min`/`std::max` throughout: without this MSVC reports
            // a hundred `C2589: '(': illegal token on right side of '::'`.
            build.define("NOMINMAX", None);
            build.define("WIN32_LEAN_AND_MEAN", None);
            if std::env::var("TARGET").is_ok_and(|target| target.ends_with("-windows-gnullvm")) {
                // LLVM-MinGW provides libc++, not libstdc++. Keep the
                // playback engine self-contained on Windows installations.
                build.cpp_link_stdlib("c++").cpp_link_stdlib_static(true);
                let output = build
                    .try_get_compiler()?
                    .to_command()
                    .arg("-print-file-name=libc++.a")
                    .output()?;
                let library = String::from_utf8(output.stdout)?;
                let library = std::path::Path::new(library.trim());
                if !output.status.success() || !library.is_file() {
                    return Err(std::io::Error::other("LLVM-MinGW static libc++ is missing").into());
                }
                let directory = library.parent().ok_or_else(|| {
                    std::io::Error::other("LLVM-MinGW static libc++ path has no parent directory")
                })?;
                println!(
                    "cargo:rustc-link-search=native={}",
                    directory.display()
                );
                println!("cargo:rustc-link-lib=static=unwind");
            }
        }
        Ok("macos") => {
            // The single-file build uses vDSP for its FFT on Apple platforms.
            println!("cargo:rustc-link-lib=framework=Accelerate");
        }
        _ => {}
    }

    build.try_compile("rubberband")?;
    Ok(())
}
