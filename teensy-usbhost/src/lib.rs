#![no_std]

//! USB Host MIDI wrapper for Teensy 4.1
//!
//! This crate provides Rust bindings to the USBHost_t36 C++ library for
//! using USB MIDI devices with Teensy 4.1 microcontrollers.
//!
//! ## Usage
//!
//! The C++ USB host library requires several callbacks from your Rust code:
//! - `rust_log_info(msg)` - logging callback
//! - `rust_micros()` - microsecond timer callback
//!
//! You must also call the ISR handler from your USB interrupt:
//! ```rust,ignore
//! #[task(binds = USB_OTG2, priority = 2)]
//! fn usb_host_isr(_cx: usb_host_isr::Context) {
//!     unsafe {
//!         teensy_usbhost::usb_isr();
//!     }
//! }
//! ```

// Link to C++ library functions
extern "C" {
    fn cpp_init();
    fn cpp_task();
    fn cpp_usb_isr();
    fn cpp_send_note_on(note: u8, velocity: u8, channel: u8);
    fn cpp_send_note_off(note: u8, velocity: u8, channel: u8);
    fn cpp_midi_connected() -> i32;
    fn cpp_midi_get_device_info(vendor: *mut u16, product: *mut u16);
}

/// Initialize the USB host subsystem.
///
/// **IMPORTANT:** Must be called after USB logging is ready (typically after a 3s delay).
/// The C++ code uses `rust_log_info()` during initialization.
///
/// # Safety
/// - Must only be called once
/// - Must be called after USB logging is initialized
pub unsafe fn init() {
    cpp_init();
}

/// Drive the USB host state machine.
///
/// **CRITICAL:** Must be called continuously in a loop to:
/// - Process USB enumeration
/// - Queue and transmit USB transfers (including MIDI TX)
/// - Handle timer events for batched MIDI sends
/// - Service RX data
///
/// Without regular calls to this function, MIDI messages will be queued but never transmitted!
///
/// # Safety
/// Must be called from the same context that initialized the USB host.
pub unsafe fn task() {
    cpp_task();
}

/// USB host interrupt service routine.
///
/// Call this from your USB_OTG2 interrupt handler.
///
/// # Safety
/// Must be called from interrupt context.
pub unsafe fn usb_isr() {
    cpp_usb_isr();
}

/// Send a MIDI Note On message.
///
/// # Safety
/// - Device must be connected (check with `midi_connected()` first)
/// - `task()` must be called regularly for the message to be transmitted
pub unsafe fn send_note_on(note: u8, velocity: u8, channel: u8) {
    cpp_send_note_on(note, velocity, channel);
}

/// Send a MIDI Note Off message.
///
/// # Safety
/// - Device must be connected (check with `midi_connected()` first)
/// - `task()` must be called regularly for the message to be transmitted
pub unsafe fn send_note_off(note: u8, velocity: u8, channel: u8) {
    cpp_send_note_off(note, velocity, channel);
}

/// Check if a MIDI device is connected.
///
/// # Returns
/// `true` if a MIDI device is connected and ready, `false` otherwise.
///
/// # Safety
/// Safe to call at any time after `init()`.
pub unsafe fn midi_connected() -> bool {
    cpp_midi_connected() == 1
}

/// Get device information for the connected MIDI device.
///
/// # Returns
/// `(vendor_id, product_id)` tuple, or `(0, 0)` if no device is connected.
///
/// # Safety
/// Safe to call at any time after `init()`.
pub unsafe fn get_device_info() -> (u16, u16) {
    let mut vendor: u16 = 0;
    let mut product: u16 = 0;
    cpp_midi_get_device_info(&mut vendor, &mut product);
    (vendor, product)
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
