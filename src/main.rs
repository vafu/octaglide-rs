#![cfg_attr(feature = "teensy", no_std)]
#![cfg_attr(feature = "teensy", no_main)]

#[cfg(not(any(feature = "teensy", feature = "host")))]
compile_error!("select exactly one platform feature: teensy or host");

#[cfg(all(feature = "teensy", feature = "host"))]
compile_error!("teensy and host features are mutually exclusive");

#[cfg(feature = "teensy")]
mod app_event;
#[cfg(feature = "teensy")]
mod calc;
#[cfg(feature = "teensy")]
mod core;
#[cfg(feature = "teensy")]
mod midi;
#[cfg(feature = "teensy")]
mod notes;
#[cfg(feature = "teensy")]
mod output;
#[cfg(feature = "teensy")]
mod runtime;
#[cfg(feature = "teensy")]
mod state;
#[cfg(feature = "teensy")]
mod teensy;
#[cfg(feature = "teensy")]
mod ui;

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
mod app_event;
#[cfg(feature = "host")]
mod calc;
#[cfg(feature = "host")]
mod core;
#[cfg(feature = "host")]
mod host;
#[cfg(feature = "host")]
mod midi;
#[cfg(feature = "host")]
mod notes;
#[cfg(feature = "host")]
mod output;
#[cfg(feature = "host")]
mod runtime;
#[cfg(feature = "host")]
mod state;
#[cfg(feature = "host")]
mod ui;

#[cfg(feature = "host")]
fn main() {
    host::run()
}
