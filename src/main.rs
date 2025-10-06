#![no_std]
#![no_main]

use teensy4_panic as _;

mod midi;
mod processor;

#[rtic::app(device = teensy4_bsp, peripherals = true, dispatchers = [KPP])]
mod app {

    use crate::{midi::MidiBus, processor::Engine};
    use board::t40 as brd;
    use embedded_alloc::LlffHeap as Heap;
    use imxrt_log as logging;
    use midi_msg::{MidiMsg, ParseError};
    use teensy4_bsp::{
        board,
        hal::{
            // gpio::Output,
            iomuxc::{
                consts::Const,
                lpuart::{self, Pin, Rx, Tx},
            },
            lpuart::{Interrupts, Lpuart, Pins},
        },
        pins::t40 as pins,
        ral::lpuart::Instance,
    };

    pub type MidiUart = Lpuart<Pins<pins::P1, pins::P0>, 6>;

    const HEAP_SIZE: usize = 1024;
    const MIDI_BAUD: u32 = 31250;

    #[shared]
    struct Shared {
        midi_bus: MidiBus,
    }

    #[local]
    struct Local {
        led: board::Led,
        // led_midi_out: Output<pins::P12>,
        poller: logging::Poller,
        engine: Engine,
    }

    #[global_allocator]
    static HEAP: Heap = Heap::empty();
    fn init_heap() {
        use core::mem::MaybeUninit;
        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe { HEAP.init(&raw mut HEAP_MEM as usize, HEAP_SIZE) }
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
        init_heap();
        rtic_monotonics::systick::Systick::start(
            cx.core.SYST,
            board::ARM_FREQUENCY,
            rtic_monotonics::create_systick_token!(),
        );

        (
            Shared {
                midi_bus: MidiBus::new(prepare_uart(lpuart6, pins.p1, pins.p0), |res| {
                    process_message::spawn(res).unwrap();
                }),
            },
            Local {
                led: board::led(&mut gpio2, pins.p13),
                // led_midi_out: gpio2.output(pins.p12),
                poller: logging::log::usbd(usb, logging::Interrupts::Enabled).unwrap(),
                engine: Engine::new(|msg| {
                    send_message::spawn(msg).unwrap();
                }),
            },
        )
    }
    fn prepare_uart<const N: u8, TX, RX>(
        instance: Instance<N>,
        tx: TX,
        rx: RX,
    ) -> Lpuart<Pins<TX, RX>, N>
    where
        TX: Pin<Direction = Tx, Module = Const<N>>,
        RX: lpuart::Pin<Direction = Rx, Module = Const<N>>,
    {
        let mut midi_uart = Lpuart::new(instance, Pins { tx: tx, rx: rx });
        midi_uart.disable(|uart| {
            uart.set_baud(&board::lpuart_baud(MIDI_BAUD));
            uart.set_interrupts(Interrupts::RECEIVE_FULL);
        });
        midi_uart
    }

    #[task(binds = LPUART6, shared = [midi_bus])]
    fn midi_handler(mut cx: midi_handler::Context) {
        cx.shared.midi_bus.lock(|midi| midi.handle_interrupt());
    }

    #[task(shared = [midi_bus])]
    async fn send_message(mut cx: send_message::Context, msg: MidiMsg) {
        cx.shared.midi_bus.lock(|midi| midi.send(msg));
    }

    #[task(local = [engine])]
    async fn process_message(cx: process_message::Context, msg: Result<MidiMsg, ParseError>) {
        cx.local.engine.on_message(msg).await;
    }

    #[task(binds = USB_OTG1, local = [poller])]
    fn log_over_usb(cx: log_over_usb::Context) {
        cx.local.poller.poll();
    }
}
