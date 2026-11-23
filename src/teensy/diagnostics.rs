//! Live crash diagnostics.
//!
//! Nothing here is retained across a reset. The BSP's `.uninit` section reads
//! back zeroed after a reboot, so a record written there is gone before the
//! next boot can report it. Diagnostics are therefore emitted while the board
//! is still running, and a hard fault that cannot drain the USB log simply
//! leaves nothing behind.

/// Records a panic message to the live log.
pub fn panic(info: &core::panic::PanicInfo<'_>) {
    if let Some(location) = info.location() {
        log::error!(
            "panicked at {}:{}:{}: {}",
            location.file(),
            location.line(),
            location.column(),
            info.message()
        );
    } else {
        log::error!("panicked: {}", info.message());
    }
}

/// Reports the faulting exception frame and the fault status registers.
///
/// Kept to fixed-width hex with no allocation, since a fault may leave the
/// stack or the formatting machinery unusable.
pub fn fault(frame: &cortex_m_rt::ExceptionFrame) {
    let scb = unsafe { &*cortex_m::peripheral::SCB::PTR };
    log::error!(
        "hardfault pc={:08x} lr={:08x} xpsr={:08x} r0={:08x} r1={:08x} r2={:08x} r3={:08x} r12={:08x}",
        frame.pc(),
        frame.lr(),
        frame.xpsr(),
        frame.r0(),
        frame.r1(),
        frame.r2(),
        frame.r3(),
        frame.r12(),
    );
    log::error!(
        "hardfault cfsr={:08x} hfsr={:08x} mmfar={:08x} bfar={:08x}",
        scb.cfsr.read(),
        scb.hfsr.read(),
        scb.mmfar.read(),
        scb.bfar.read(),
    );
}
