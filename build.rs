// Build script for octaglide-rs
//
// Note: USB host compilation has been moved to the teensy-usbhost crate

fn main() {
    // Nothing to build here - teensy-usbhost handles C++ compilation
    println!("cargo:rerun-if-changed=build.rs");
}
