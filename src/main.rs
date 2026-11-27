#![cfg_attr(feature = "teensy", no_std)]
#![cfg_attr(feature = "teensy", no_main)]

#[cfg(not(any(feature = "teensy", feature = "host")))]
compile_error!("select exactly one platform feature: teensy or host");

#[cfg(all(feature = "teensy", feature = "host"))]
compile_error!("teensy and host features are mutually exclusive");

// Shared by both platforms; the platform modules below pick their own
// board support and entry point.
#[cfg(any(feature = "teensy", feature = "host"))]
mod app_event;
#[cfg(any(feature = "teensy", feature = "host"))]
mod core;
#[cfg(any(feature = "teensy", feature = "host"))]
mod midi;
#[cfg(any(feature = "teensy", feature = "host"))]
mod notes;
#[cfg(any(feature = "teensy", feature = "host"))]
mod output;
#[cfg(any(feature = "teensy", feature = "host"))]
mod runtime;
#[cfg(any(feature = "teensy", feature = "host"))]
mod state;
#[cfg(any(feature = "teensy", feature = "host"))]
mod ui;

#[cfg(feature = "teensy")]
mod teensy;

#[cfg(feature = "teensy")]
extern crate alloc;

/// RTIC app module, re-exported at the path the board code expects.
#[cfg(feature = "teensy")]
pub use teensy::app;

#[cfg(feature = "teensy")]
mod firmware {
    use ::core::panic::PanicInfo;
    use cortex_m::interrupt;

    #[panic_handler]
    #[allow(static_mut_refs)]
    fn panic(info: &PanicInfo) -> ! {
        interrupt::disable();
        crate::teensy::diagnostics::panic(info);
        log::set_max_level(log::LevelFilter::Error);
        unsafe {
            if let Some(poller) = crate::app::POLLER.as_mut() {
                for _ in 0..5000 {
                    poller.poll();
                    cortex_m::asm::delay(10000);
                }

                teensy4_panic::sos()
            } else {
                loop {}
            }
        }
    }

    /// Records the fault before parking, so the next boot can report it.
    ///
    /// Without this the default handler is a bare infinite loop, which is
    /// indistinguishable from a hang in the vendored USB host code.
    #[cortex_m_rt::exception]
    #[allow(static_mut_refs)]
    unsafe fn HardFault(frame: &cortex_m_rt::ExceptionFrame) -> ! {
        interrupt::disable();
        crate::teensy::diagnostics::fault(frame);
        log::set_max_level(log::LevelFilter::Error);
        unsafe {
            if let Some(poller) = crate::app::POLLER.as_mut() {
                for _ in 0..5000 {
                    poller.poll();
                    cortex_m::asm::delay(10000);
                }
            }
        }
        teensy4_panic::sos()
    }
}

#[cfg(feature = "host")]
mod host;

#[cfg(feature = "host")]
fn main() {
    host::run()
}
