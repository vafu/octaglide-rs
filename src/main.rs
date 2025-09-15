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
        hal::lpuart::{Interrupts, Lpuart, Pins},
        pins::t40,
    };

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
        /// A poller to control USB logging.
        poller: logging::Poller,
        midi_uart: Lpuart<Pins<t40::P14, t40::P15>, 2>,
        midi_ctx: ReceiverContext,
        midi_buffer: Vec<u8, 64>,
    }

    #[init]
    fn init(cx: init::Context) -> (Shared, Local) {
        let board::Resources {
            mut gpio2,
            pins,
            usb,
            lpuart2,
            ..
        } = brd(cx.device);

        {
            use core::mem::MaybeUninit;
            const HEAP_SIZE: usize = 1024;
            static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
            unsafe { HEAP.init(&raw mut HEAP_MEM as usize, HEAP_SIZE) }
        }

        let led = board::led(&mut gpio2, pins.p13);
        let poller = logging::log::usbd(usb, logging::Interrupts::Enabled).unwrap();
        let mut midi_uart: board::Lpuart2 = board::Lpuart2::new(
            lpuart2,
            Pins {
                tx: pins.p14,
                rx: pins.p15,
            },
        );
        midi_uart.disable(|uart| {
            uart.set_baud(&board::lpuart_baud(31250));
            uart.set_interrupts(Interrupts::RECEIVE_FULL);
        });

        let midi_ctx = ReceiverContext::new();
        let midi_buffer = Vec::new();
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
            },
        )
    }

    #[task(binds = LPUART2, local = [midi_uart, midi_ctx, midi_buffer])]
    fn midi_handler(cx: midi_handler::Context) {
        let uart = cx.local.midi_uart;
        let ctx = cx.local.midi_ctx;
        let buffer = cx.local.midi_buffer;

        let mut cnt = 0;
        while let Ok(Some(byte)) = uart.try_read() {
            buffer.push(byte).ok();
            cnt += 1;
        }

        log::info!("Done reading : {:?} bytes", cnt);

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
