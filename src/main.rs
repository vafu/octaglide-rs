#![no_std]
#![no_main]

use teensy4_panic as _;

mod core;
mod midi;
mod processor;

#[macro_use]
extern crate alloc;

#[rtic::app(device = teensy4_bsp, peripherals = true, dispatchers = [KPP])]
mod app {

    use crate::{
        core::{Core, Input as CoreIn, Output as CoreOut},
        midi::MidiBus,
    };
    use board::t40 as brd;
    use embedded_alloc::LlffHeap as Heap;
    use imxrt_log as logging;
    use midi_msg::MidiMsg;
    use rtic_monotonics::systick::{Systick, *};
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
        poller: logging::Poller,
        engine: Core,
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
                    process_input::spawn(CoreIn::ProcessMidi(res)).unwrap();
                }),
            },
            Local {
                led: board::led(&mut gpio2, pins.p13),
                poller: logging::log::usbd(usb, logging::Interrupts::Enabled).unwrap(),
                engine: Core::new(dispatch_output),
            },
        )
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            cortex_m::asm::wfi();
        }
    }

    fn dispatch_output(output: CoreOut) {
        match output {
            CoreOut::SendMidi(msg) => {
                send_message::spawn(msg).unwrap();
            }
            CoreOut::BlinkLed => {
                blink_led::spawn().ok();
            }
        }
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
        let mut midi_uart = Lpuart::new(instance, Pins { tx, rx });
        midi_uart.disable(|uart| {
            uart.set_baud(&board::lpuart_baud(MIDI_BAUD));
            uart.set_interrupts(Interrupts::RECEIVE_FULL);
        });
        midi_uart
    }

    // TODO: figure out priorities
    #[task(binds = LPUART6, shared = [midi_bus], priority = 1)]
    fn midi_handler(mut cx: midi_handler::Context) {
        cx.shared.midi_bus.lock(|midi| midi.handle_interrupt());
    }

    #[task(shared = [midi_bus], priority = 1)]
    async fn send_message(mut cx: send_message::Context, msg: MidiMsg) {
        cx.shared.midi_bus.lock(|midi| midi.send(msg));
    }

    #[task(local = [engine], priority = 1)]
    async fn process_input(cx: process_input::Context, input: CoreIn) {
        cx.local.engine.process(input).await;
    }

    #[task(local = [led], priority = 1)]
    async fn blink_led(cx: blink_led::Context) {
        let led = cx.local.led;
        led.set();
        Systick::delay(1.millis()).await;
        led.clear();
    }

    #[task(binds = USB_OTG1, local = [poller])]
    fn log_over_usb(cx: log_over_usb::Context) {
        cx.local.poller.poll();
    }
}
