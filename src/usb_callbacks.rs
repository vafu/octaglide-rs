//! C++ callback implementations for USB host library
//!
//! These callbacks must be defined in the main application because:
//! - rust_log_info: Needs access to the logger initialized in main.rs
//! - rust_micros: TODO task-24 - Should be in teensy-usbhost but callback indirection breaks USB

use rtic_monotonics::{Monotonic, systick::Systick};

/// C++ callback for microsecond timer
///
/// TODO: task-24 - Move this to teensy-usbhost crate once callback issue is fixed
#[unsafe(no_mangle)]
pub extern "C" fn rust_micros() -> u32 {
    Systick::now().duration_since_epoch().to_micros()
}

/// C++ callback for logging
///
/// Must be here in main app where the logger is initialized
#[unsafe(no_mangle)]
pub extern "C" fn rust_log_info(msg: *const u8) {
    use ::core::{slice, str};
    if !msg.is_null() {
        unsafe {
            let mut len = 0;
            while *msg.add(len) != 0 {
                len += 1;
                if len > 1024 {
                    // Safety limit
                    return;
                }
            }
            if let Ok(s) = str::from_utf8(slice::from_raw_parts(msg, len)) {
                log::info!("[C++] {}", s);
            }
        }
    }
}
