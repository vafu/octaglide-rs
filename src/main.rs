#![no_std]
#![no_main]

use ::core::panic::PanicInfo;

use cortex_m::interrupt;

#[panic_handler]
#[allow(static_mut_refs)]
fn panic(info: &PanicInfo) -> ! {
    cortex_m::interrupt::disable();
    log::error!("{}", info);
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

mod core;
mod engine;
mod midi;
mod processor;

#[macro_use]
extern crate alloc;

#[rtic::app(device = teensy4_bsp, peripherals = true, dispatchers = [KPP, GPT1])]
mod app {

    use crate::{
        core::{Core, Input as CoreIn, Output as CoreOut},
        engine::{Engine, EngineMessage},
        midi::MidiBus,
    };
    use board::t40 as brd;
    use embedded_alloc::LlffHeap as Heap;
    use imxrt_log as logging;
    use midi_msg::{Channel, MidiMsg};
    use rtic_monotonics::systick::{Systick, *};
    use rtic_sync::{
        channel::{Receiver, Sender},
        make_channel,
    };
    use teensy4_bsp::{
        board,
        hal::{
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
    const MIDI_CHANNEL_CAPACITY: usize = 16;

    pub(crate) static mut POLLER: Option<logging::Poller> = None;

    #[shared]
    struct Shared {
        midi_bus: MidiBus,
    }

    #[local]
    struct Local {
        led: board::Led,
        // poller: logging::Poller,
        engine: Core,
    }

    pub struct Dispatcher {
        midi_sender: Sender<'static, MidiMsg, MIDI_CHANNEL_CAPACITY>,
        engine_sender: Sender<'static, EngineMessage, 1>,
    }

    impl Dispatcher {
        pub async fn dispatch(&mut self, output: CoreOut) {
            match output {
                CoreOut::SendMidi(msg) => {
                    self.midi_sender.send(msg.clone()).await.unwrap();
                }
                CoreOut::BlinkLed => {
                    blink_led::spawn().ok();
                }
                CoreOut::Engine(message) => {
                    self.engine_sender.send(message).await.unwrap();
                }
            }
        }
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
        Systick::start(
            cx.core.SYST,
            board::ARM_FREQUENCY,
            rtic_monotonics::create_systick_token!(),
        );
        let poller = logging::log::usbd(usb, logging::Interrupts::Enabled).unwrap();
        crate::interrupt::free(|_| unsafe {
            POLLER = Some(poller);
        });

        let (s, r) = make_channel!(MidiMsg, MIDI_CHANNEL_CAPACITY);
        let (engine, es) = Engine::new(Channel::Ch6, 13);

        let dispatcher = Dispatcher {
            midi_sender: s,
            engine_sender: es,
        };

        midi_dispatch::spawn(r).unwrap();
        animate::spawn(engine).unwrap();

        (
            Shared {
                midi_bus: MidiBus::new(prepare_uart(lpuart6, pins.p1, pins.p0), |res| {
                    process_input::spawn(CoreIn::ProcessMidi(res)).unwrap();
                }),
            },
            Local {
                led: board::led(&mut gpio2, pins.p13),
                engine: Core::new(dispatcher),
            },
        )
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            cortex_m::asm::wfi();
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

    #[task(binds = LPUART6, shared = [midi_bus], priority = 2)]
    fn midi_handler(mut cx: midi_handler::Context) {
        cx.shared.midi_bus.lock(|midi| midi.handle_interrupt());
    }

    #[task(shared = [midi_bus], priority = 2)]
    async fn midi_dispatch(
        mut cx: midi_dispatch::Context,
        mut r: Receiver<'static, MidiMsg, MIDI_CHANNEL_CAPACITY>,
    ) -> ! {
        loop {
            let msg = r.recv().await.unwrap();
            cx.shared.midi_bus.lock(|midi| {
                midi.send(&msg);
            });
        }
    }
    #[task(shared = [midi_bus], priority = 2)]
    async fn animate(mut ctx: animate::Context, mut engine: Engine) -> ! {
        loop {
            if let Some(msg) = engine.tick().await {
                ctx.shared.midi_bus.lock(|bus| bus.send(&msg));
            }
        }
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

    #[task(binds = USB_OTG1, priority = 2)]
    #[allow(static_mut_refs)]
    fn log_over_usb(cx: log_over_usb::Context) {
        unsafe {
            crate::interrupt::free(|_| {
                if let Some(p) = POLLER.as_mut() {
                    p.poll();
                }
            });
        }
    }
}
