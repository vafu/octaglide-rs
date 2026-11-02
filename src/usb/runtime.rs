use crate::{app::CoreSender, core::Input};
use rtic_monotonics::systick::{ExtU32, Systick};
use teensy_usbhost as usbhost;

/// Service enumeration, MIDI and disconnects for the lifetime of the USB host.
/// Only status changes enter Core's queue; retry when the queue is full.
pub async fn run_host(mut sender: CoreSender) -> ! {
    usbhost::init();
    let mut reported = None;
    let mut reported_port = None;
    loop {
        usbhost::task();
        let port = usbhost::host_port_status();
        // Report stable connection-stage bits, avoiding write-one-to-clear flags.
        let state = port & ((1 << 0) | (1 << 2) | (1 << 8) | (1 << 12));
        if reported_port != Some(state) {
            reported_port = Some(state);
            log::info!(
                "USB host port: attached={}, enabled={}, reset={}, power={}, status={:08x}",
                state & 1 != 0,
                state & (1 << 2) != 0,
                state & (1 << 8) != 0,
                state & (1 << 12) != 0,
                port,
            );
        }
        let connected = usbhost::midi_connected();
        if reported != Some(connected)
            && sender.try_send(Input::UsbConnection { connected }).is_ok()
        {
            reported = Some(connected);
            if connected {
                let (vendor, product) = usbhost::get_device_info();
                log::info!(
                    "USB MIDI device CONNECTED - VID:{:04X} PID:{:04X}",
                    vendor,
                    product
                );
            }
        }
        Systick::delay(10.millis()).await;
    }
}
