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

mod anim;
mod core;
mod midi;
mod midi_fmt;

#[macro_use]
extern crate alloc;

#[rtic::app(device = teensy4_bsp, peripherals = true, dispatchers = [KPP, GPT1])]
mod app {

    use crate::{
        anim::animator::{Animator, Cmd},
        core::{Core, Input as CoreIn, MidiEvent},
        midi::MidiBus,
    };
    use board::t40 as brd;
    use embedded_alloc::LlffHeap as Heap;
    use imxrt_log as logging;
    use midi_msg::MidiMsg;
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
    const CORE_INPUT_CHANNEL_CAPACITY: usize = 16;

    // Channel type aliases
    pub type MidiSender = Sender<'static, MidiMsg, MIDI_CHANNEL_CAPACITY>;
    pub type MidiReceiver = Receiver<'static, MidiMsg, MIDI_CHANNEL_CAPACITY>;
    pub type AnimatorSender = Sender<'static, Cmd, 1>;
    pub type CoreSender = Sender<'static, CoreIn, CORE_INPUT_CHANNEL_CAPACITY>;
    pub type CoreReceiver = Receiver<'static, CoreIn, CORE_INPUT_CHANNEL_CAPACITY>;

    pub(crate) static mut POLLER: Option<logging::Poller> = None;

    #[shared]
    struct Shared {
        midi_bus: MidiBus,
    }

    #[local]
    struct Local {
        led: board::Led,
        core: Core,
        // poller: logging::Poller,
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

        let (midi_sender, midi_receiver) = make_channel!(MidiMsg, MIDI_CHANNEL_CAPACITY);
        let (animator, animator_sender) = Animator::new();
        let (core_sender, core_receiver) = make_channel!(CoreIn, CORE_INPUT_CHANNEL_CAPACITY);

        midi_dispatch::spawn(midi_receiver).unwrap();
        animate::spawn(animator, core_sender.clone()).unwrap();
        core_task::spawn(core_receiver).ok();

        (
            Shared {
                midi_bus: MidiBus::new(
                    prepare_uart(lpuart6, pins.p1, pins.p0),
                    core_sender.clone(),
                ),
            },
            Local {
                led: board::led(&mut gpio2, pins.p13),
                core: Core::new(midi_sender, animator_sender),
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
    async fn midi_dispatch(mut cx: midi_dispatch::Context, mut r: MidiReceiver) -> ! {
        loop {
            let msg = r.recv().await.unwrap();
            cx.shared.midi_bus.lock(|midi| {
                midi.send(&msg);
            });
        }
    }
    #[task(priority = 2)]
    async fn animate(_cx: animate::Context, mut animator: Animator, mut sender: CoreSender) -> ! {
        loop {
            if let Some(msgs) = animator.tick().await {
                for msg in msgs {
                    sender
                        .send(CoreIn::Process(MidiEvent::synthetic(msg)))
                        .await
                        .ok();
                }
            }
        }
    }

    #[task(local = [core], priority = 1)]
    async fn core_task(cx: core_task::Context, mut receiver: CoreReceiver) -> ! {
        let core = cx.local.core;
        loop {
            if let Ok(input) = receiver.recv().await {
                core.process(input).await;
            }
        }
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
    fn log_over_usb(_cx: log_over_usb::Context) {
        unsafe {
            crate::interrupt::free(|_| {
                if let Some(p) = POLLER.as_mut() {
                    p.poll();
                }
            });
        }
    }
}
