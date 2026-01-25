#![no_std]

//! USB Host MIDI wrapper for Teensy 4.1
//!
//! This crate provides Rust bindings to the USBHost_t36 C++ library for
//! using USB MIDI devices with Teensy 4.1 microcontrollers.
//!
//! ## Usage
//!
//! Before initializing the USB host, you must set up a time source:
//!
//! ```rust,ignore
//! teensy_usbhost::set_time_source(|| {
//!     Systick::now().duration_since_epoch().to_micros()
//! });
//!
//! teensy_usbhost::init();
//! ```
//!
//! Then call the ISR handler from your USB interrupt:
//! ```rust,ignore
//! #[task(binds = USB_OTG2, priority = 2)]
//! fn usb_host_isr(_cx: usb_host_isr::Context) {
//!     unsafe {
//!         teensy_usbhost::usb_isr();
//!     }
//! }
//! ```

use core::sync::atomic::{AtomicPtr, Ordering};

extern "C" {
    fn cpp_init();
    fn cpp_task();
    fn cpp_usb_isr();
    fn cpp_send_note_on(note: u8, velocity: u8, channel: u8);
    fn cpp_send_note_off(note: u8, velocity: u8, channel: u8);
    fn cpp_midi_connected() -> i32;
    fn cpp_midi_get_device_info(vendor: *mut u16, product: *mut u16);
}

// --- TIME SOURCE CALLBACK ---

static TIME_SOURCE: AtomicPtr<()> = AtomicPtr::new(core::ptr::null_mut());

/// Set the microsecond time source for the USB host library.
///
/// This must be called before `init()` to provide a function that returns
/// the current time in microseconds.
///
/// # Example
/// ```rust,ignore
/// teensy_usbhost::set_time_source(|| {
///     Systick::now().duration_since_epoch().to_micros()
/// });
/// ```
pub fn set_time_source(time_fn: fn() -> u32) {
    TIME_SOURCE.store(time_fn as *mut (), Ordering::Release);
}

fn get_micros() -> u32 {
    let ptr = TIME_SOURCE.load(Ordering::Acquire);
    if ptr.is_null() {
        // No time source set - return 0 (this will likely cause issues)
        0
    } else {
        let time_fn: fn() -> u32 = unsafe { core::mem::transmute(ptr) };
        time_fn()
    }
}

/// Initialize the USB host subsystem.
///
/// **IMPORTANT:**
/// - Must be called after `set_time_source()`
/// - Must be called after USB logging is ready (typically after a 3s delay)
///
/// # Safety
/// - Must only be called once
/// - Must be called after `set_time_source()` has been called
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

// --- C++ CALLBACK IMPLEMENTATIONS ---

/// C++ callback for microsecond timer
///
/// This is called by the C++ USB host library to get the current time.
#[no_mangle]
pub extern "C" fn rust_micros() -> u32 {
    get_micros()
}

/// C++ callback for logging
///
/// This is called by the C++ USB host library to log messages.
#[no_mangle]
pub extern "C" fn rust_log_info(msg: *const u8) {
    use core::{slice, str};

    if msg.is_null() {
        return;
    }

    // Find null terminator
    let mut len = 0;
    unsafe {
        while *msg.add(len) != 0 {
            len += 1;
            if len > 1024 {
                // Safety limit
                return;
            }
        }
    }

    // Convert to Rust string slice
    let bytes = unsafe { slice::from_raw_parts(msg, len) };
    if let Ok(s) = str::from_utf8(bytes) {
        log::info!("{}", s);
    }
}
