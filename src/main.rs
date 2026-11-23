#![no_std]
#![no_main]

use ::core::panic::PanicInfo;
use cortex_m::interrupt;

#[panic_handler]
#[allow(static_mut_refs)]
fn panic(info: &PanicInfo) -> ! {
    interrupt::disable();
    crate::teensy::diagnostics::panic(info);
    log::set_max_level(log::LevelFilter::Error);
    unsafe {
        if let Some(poller) = app::POLLER.as_mut() {
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
        if let Some(poller) = app::POLLER.as_mut() {
            for _ in 0..5000 {
                poller.poll();
                cortex_m::asm::delay(10000);
            }
        }
    }
    teensy4_panic::sos()
}

mod app_event;
mod calc;
mod core;
mod midi;
mod notes;
mod output;
mod runtime;
mod state;
mod teensy;
mod ui;

/// RTIC app module, re-exported at the path the board code expects.
pub use teensy::app;

extern crate alloc;
