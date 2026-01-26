#![no_std]

//! USB Host MIDI wrapper for Teensy 4.1
//!
//! This crate provides Rust bindings to the USBHost_t36 C++ library for
//! using USB MIDI devices with Teensy 4.1 microcontrollers.
//!
//! ## Usage
//!
//! The main app must provide these C callbacks (marked with `#[unsafe(no_mangle)]`):
//! - `rust_log_info(msg: *const u8)` - logging callback
//! - `rust_micros() -> u32` - microsecond timer callback
//!
//! Call the ISR handler from your USB interrupt:
//! ```rust,ignore
//! #[task(binds = USB_OTG2, priority = 2)]
//! fn usb_host_isr(_cx: usb_host_isr::Context) {
//!         teensy_usbhost::usb_isr();
//! }
//! ```

extern "C" {
    fn cpp_init();
    fn cpp_task();
    fn cpp_usb_isr();
    fn cpp_send_midi(data: *const u8, len: u8);
    fn cpp_midi_connected() -> i32;
    fn cpp_midi_get_device_info(vendor: *mut u16, product: *mut u16);
}

// TODO: task-24 - Fix callback indirection breaking USB enumeration
// This callback mechanism works but breaks USB for unknown reasons.
// For now, rust_micros() must be defined directly in main.rs.
//
// --- TIME SOURCE CALLBACK (CURRENTLY DISABLED) ---
//
// static mut TIME_SOURCE: Option<fn() -> u32> = None;
//
// pub unsafe fn set_time_source(time_fn: fn() -> u32) {
//     TIME_SOURCE = Some(time_fn);
// }
//
// fn get_micros() -> u32 {
//     unsafe {
//         if let Some(time_fn) = TIME_SOURCE {
//             time_fn()
//         } else {
//             0
//         }
//     }
// }
//
// #[unsafe(no_mangle)]
// pub extern "C" fn rust_micros() -> u32 {
//     get_micros()
// }

/// Initialize the USB host subsystem.
///
/// **IMPORTANT:** Must be called after USB logging is ready (typically after a 3s delay).
///
/// # Safety
/// - Must only be called once
/// - Must be called after USB logging is initialized
pub fn init() {
    unsafe {
        cpp_init();
    }
}

/// Drive the USB host state machine.
///
/// **CRITICAL:** Must be called regularly to:
/// - Process USB enumeration
/// - Queue and transmit USB transfers (including MIDI TX)
/// - Handle timer events for batched MIDI sends
/// - Service RX data
///
/// Recommended calling patterns:
/// - From USB interrupt handler (event-driven)
/// - After every send operation (immediate TX)
/// - Periodic maintenance task at ~10ms intervals (timer events)
///
/// Without regular calls to this function, MIDI messages will be queued but never transmitted!
///
/// # Safety
/// Must be called from the same context that initialized the USB host.
pub fn task() {
    unsafe {
        cpp_task();
    }
}

/// USB host interrupt service routine.
///
/// Call this from your USB_OTG2 interrupt handler.
///
/// # Safety
/// Must be called from interrupt context.
pub fn usb_isr() {
    unsafe {
        cpp_usb_isr();
    }
}

/// Check if a MIDI device is connected.
///
/// # Returns
/// `true` if a MIDI device is connected and ready, `false` otherwise.
///
/// # Safety
/// Safe to call at any time after `init()`.
pub fn midi_connected() -> bool {
    unsafe { cpp_midi_connected() == 1 }
}

/// Get device information for the connected MIDI device.
///
/// # Returns
/// `(vendor_id, product_id)` tuple, or `(0, 0)` if no device is connected.
///
/// # Safety
/// Safe to call at any time after `init()`.
pub fn get_device_info() -> (u16, u16) {
    let mut vendor: u16 = 0;
    let mut product: u16 = 0;
    unsafe {
        cpp_midi_get_device_info(&mut vendor, &mut product);
    }
    (vendor, product)
}

/// Send a generic MIDI message.
///
/// This function accepts any MIDI message and dispatches it to the appropriate
/// USB MIDI method based on the status byte.
///
/// # Safety
/// - Device must be connected (check with `midi_connected()` first)
/// - `task()` must be called regularly for the message to be transmitted
/// - The message must be a valid MIDI message with proper status byte
pub fn send_midi(msg: &midi_msg::MidiMsg) {
    let bytes = msg.to_midi();
    if !bytes.is_empty() && bytes.len() <= 16 {
        unsafe {
            cpp_send_midi(bytes.as_ptr(), bytes.len() as u8);
        }
    }
    task();
}

/// Device information structure
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceInfo {
    pub vendor_id: u16,
    pub product_id: u16,
}

impl DeviceInfo {
    /// Get current device info, or None if no device connected
    pub fn current() -> Option<Self> {
        unsafe {
            if midi_connected() {
                let (vendor_id, product_id) = get_device_info();
                Some(DeviceInfo {
                    vendor_id,
                    product_id,
                })
            } else {
                None
            }
        }
    }
}
