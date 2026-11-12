// Build script for octaglide-rs
//
// Note: USB host compilation has been moved to the teensy-usbhost crate

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=OCTAGLIDE_LOG");

    let level = std::env::var("OCTAGLIDE_LOG").unwrap_or_else(|_| "info".to_owned());
    if !matches!(level.as_str(), "off" | "error" | "warn" | "info" | "debug" | "trace") {
        panic!("OCTAGLIDE_LOG must be one of: off, error, warn, info, debug, trace");
    }
    println!("cargo:rustc-env=OCTAGLIDE_LOG={level}");
}
