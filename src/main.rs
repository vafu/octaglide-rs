#![no_std]
#![no_main]

use teensy4_panic as _;
#[rtic::app(device = teensy4_bsp, peripherals = true, dispatchers = [KPP])]
mod app {
    use bsp::board;
    use embedded_alloc::LlffHeap as Heap;
    use heapless::Vec;
    use midi_msg::{self, MidiMsg, ReceiverContext};
    use teensy4_bsp::{
        self as bsp,
        hal::{
            gpio::Output,
            lpuart::{Interrupts, Lpuart, Pins},
        },
        pins::t40,
    };

    const HEAP_SIZE: usize = 1024;
    const MIDI_BUF_SIZE: usize = 32;
    const MIDI_BAUD: u32 = 31250;

    #[global_allocator]
    static HEAP: Heap = Heap::empty();

    use imxrt_log as logging;

    use board::t40 as brd;

    use rtic_monotonics::systick::{Systick, *};

    /// There are no resources shared across tasks.
    #[shared]
    struct Shared {}

    /// These resources are local to individual tasks.
    #[local]
    struct Local {
        /// The LED on pin 13.
        led: board::Led,
        led_midi_out: Output<t40::P12>,
        /// A poller to control USB logging.
        poller: logging::Poller,
        midi_uart: Lpuart<Pins<t40::P1, t40::P0>, 6>,
        midi_ctx: ReceiverContext,
        midi_out_ctx: Option<u8>,
        midi_buffer: Vec<u8, MIDI_BUF_SIZE>,
    }

    #[init]
    fn init(cx: init::Context) -> (Shared, Local) {
        let board::Resources {
            mut gpio2,
            pins,
            usb,
            lpuart6,
            ..
        } = brd(cx.device);

        {
            use core::mem::MaybeUninit;
            static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
            unsafe { HEAP.init(&raw mut HEAP_MEM as usize, HEAP_SIZE) }
        }

        let led = board::led(&mut gpio2, pins.p13);
        let poller = logging::log::usbd(usb, logging::Interrupts::Enabled).unwrap();
        let mut midi_uart: board::Lpuart6 = board::Lpuart6::new(
            lpuart6,
            Pins {
                tx: pins.p1,
                rx: pins.p0,
            },
        );
        midi_uart.disable(|uart| {
            uart.set_baud(&board::lpuart_baud(MIDI_BAUD));
            uart.set_interrupts(Interrupts::RECEIVE_FULL);
        });

        let midi_ctx = ReceiverContext::new();
        let midi_buffer = Vec::new();
        let led_midi_out = gpio2.output(pins.p12);

        Systick::start(
            cx.core.SYST,
            board::ARM_FREQUENCY,
            rtic_monotonics::create_systick_token!(),
        );
        (
            Shared {},
            Local {
                led,
                poller,
                midi_uart,
                midi_ctx,
                midi_buffer,
                led_midi_out,
                midi_out_ctx: None,
            },
        )
    }

    #[task(binds = LPUART6, local = [midi_uart, midi_ctx, midi_buffer, led_midi_out, midi_out_ctx])]
    fn midi_handler(cx: midi_handler::Context) {
        let uart = cx.local.midi_uart;
        let ctx = cx.local.midi_ctx;
        let buffer = cx.local.midi_buffer;
        let led_midi_out = cx.local.led_midi_out;
        let tx_ctx = cx.local.midi_out_ctx;

        while let Ok(Some(byte)) = uart.try_read() {
            buffer.push(byte).ok();
        }

        let mut consumed = 0;
        loop {
            let slice_to_parse = &buffer[consumed..];
            if slice_to_parse.is_empty() {
                break;
            }

            match MidiMsg::from_midi_with_context(slice_to_parse, ctx) {
                Ok((msg, len)) => {
                    blink_led::spawn().ok();
                    log::info!("Received: {:?}", msg);

                    // TODO: simplify into one method.

                    led_midi_out.set();

                    let to_send = msg.to_midi();
                    let is_channel_msg = msg.is_channel_mode();
                    let status = to_send[0];

                    if is_channel_msg && *tx_ctx == Some(status) {
                        for &byte in &to_send[1..] {
                            while !uart.try_write(byte) {
                                core::hint::spin_loop();
                            }
                        }
                    } else {
                        for byte in msg.to_midi() {
                            while !uart.try_write(byte) {
                                core::hint::spin_loop();
                            }
                        }
                        if is_channel_msg {
                            *tx_ctx = Some(status);
                        }
                    }
                    led_midi_out.clear();

                    consumed += len;
                }
                Err(midi_msg::ParseError::UnexpectedEnd) => {
                    break;
                }
                Err(e) => {
                    log::warn!("MIDI Parse Error: {:?}. Discarding a byte.", e);
                    consumed += 1;
                }
            }
        }

        if consumed > 0 {
            let original_len = buffer.len();
            buffer.copy_within(consumed.., 0);
            buffer.truncate(original_len - consumed);
        }
    }

    #[task(local = [led], priority = 1)]
    async fn blink_led(cx: blink_led::Context) {
        cx.local.led.set();
        Systick::delay(10.millis()).await;
        cx.local.led.clear();
    }

    #[task(binds = USB_OTG1, local = [poller])]
    fn log_over_usb(cx: log_over_usb::Context) {
        cx.local.poller.poll();
    }
}
