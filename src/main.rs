#![no_std]
#![no_main]

use ::core::panic::PanicInfo;
use cortex_m::interrupt;

#[panic_handler]
#[allow(static_mut_refs)]
fn panic(info: &PanicInfo) -> ! {
    interrupt::disable();
    log::error!("{}", info);
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

mod anim;
mod core;
mod midi;
mod midi_fmt;
mod usb_callbacks;
mod usb_midi;

extern crate alloc;

#[rtic::app(device = teensy4_bsp, peripherals = true, dispatchers = [KPP, GPT1])]
mod app {

    use crate::{
        anim::animator::{AnimationEngine, Cmd},
        core::{Core, Input as CoreIn, MidiEvent},
        midi::{Bus, MidiBus, UartMidiBus},
        midi_fmt::MidiFmt,
        usb_midi::UsbMidiBus,
    };
    use board::t41 as brd;
    use embedded_alloc::LlffHeap as Heap;
    use imxrt_log as logging;
    use log::info;
    use midi_msg::MidiMsg;
    use rtic_monotonics::systick::{Systick, *};
    use rtic_sync::{
        channel::{Receiver, Sender},
        make_channel,
    };
    use teensy_usbhost as usbhost;
    use teensy4_bsp::{
        board,
        hal::{
            gpio,
            iomuxc::{
                self,
                consts::Const,
                lpuart::{self, Pin, Rx, Tx},
            },
            lpuart::{Interrupts, Lpuart, Pins},
        },
        pins::t40 as pins,
        ral::lpuart::Instance,
    };

    pub type MidiUart = Lpuart<Pins<pins::P1, pins::P0>, 6>;
    pub type MidiSender = Sender<'static, MidiMsg, MIDI_CHANNEL_CAPACITY>;
    pub type MidiReceiver = Receiver<'static, MidiMsg, MIDI_CHANNEL_CAPACITY>;
    pub type Animator = Sender<'static, Cmd, 1>;
    pub type CoreSender = Sender<'static, CoreIn, CORE_INPUT_CHANNEL_CAPACITY>;
    pub type CoreReceiver = Receiver<'static, CoreIn, CORE_INPUT_CHANNEL_CAPACITY>;

    macro_rules! make_animator {
        ($task_name:ident, $core_sender:expr) => {{
            let (animator, cmd_rx) = make_channel!(Cmd, 1);
            let engine = AnimationEngine::new(cmd_rx);
            $task_name::spawn(engine, $core_sender.clone()).unwrap();
            animator
        }};
    }

    async fn run_animator_loop(mut engine: AnimationEngine, mut sender: CoreSender) -> ! {
        loop {
            if let Some(msgs) = engine.tick().await {
                for msg in msgs {
                    sender
                        .send(CoreIn::Process(MidiEvent::synthetic(msg)))
                        .await
                        .ok();
                }
            }
        }
    }

    const HEAP_SIZE: usize = 1024;
    const MIDI_BAUD: u32 = 31250;
    const MIDI_CHANNEL_CAPACITY: usize = 16;
    const CORE_INPUT_CHANNEL_CAPACITY: usize = 16;

    pub(crate) static mut POLLER: Option<logging::Poller> = None;

    #[shared]
    struct Shared {
        rx_bus: Bus, // UART input
        tx_bus: Bus, // USB output
    }

    #[local]
    struct Local {
        led: board::Led,
        core: Core,
        reset_btn: gpio::Input<pins::P14>,
    }

    #[global_allocator]
    static HEAP: Heap = Heap::empty();
    fn init_heap() {
        use core::mem::MaybeUninit;
        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe { HEAP.init(&raw mut HEAP_MEM as usize, HEAP_SIZE) }
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

    #[init]
    #[allow(static_mut_refs)]
    fn init(cx: init::Context) -> (Shared, Local) {
        init_heap();
        Systick::start(
            cx.core.SYST,
            board::ARM_FREQUENCY,
            rtic_monotonics::create_systick_token!(),
        );

        let board::Resources {
            mut gpio2,
            mut gpio1,
            mut pins,
            usb,
            lpuart6,
            ..
        } = brd(cx.device);

        let poller = logging::log::usbd(usb, logging::Interrupts::Enabled).unwrap();
        crate::interrupt::free(|_| unsafe {
            POLLER = Some(poller);
        });

        let (midi_sender, midi_receiver) = make_channel!(MidiMsg, MIDI_CHANNEL_CAPACITY);
        let (core_sender, core_receiver) = make_channel!(CoreIn, CORE_INPUT_CHANNEL_CAPACITY);

        let glide_animator = make_animator!(animate_glide, core_sender);
        let envelope_animator = make_animator!(animate_envelope, core_sender);

        // Configure reset button (P14) with pulldown for interrupt on rising edge (button press to 3.3V)
        // Wire momentary button from P14 to 3.3V
        iomuxc::configure(
            &mut pins.p14,
            iomuxc::Config::zero().set_pull_keeper(Some(iomuxc::PullKeeper::Pullup100k)),
        );
        let reset_btn = gpio1.input(pins.p14);

        // Enable GPIO interrupt on rising edge (button press)
        gpio1.set_interrupt(&reset_btn, Some(gpio::Trigger::RisingEdge));

        midi_dispatch::spawn(midi_receiver).unwrap();
        core_task::spawn(core_receiver).unwrap();
        usb_host_init::spawn().unwrap();

        // Create UART bus for RX
        let uart_bus =
            UartMidiBus::new(prepare_uart(lpuart6, pins.p1, pins.p0), core_sender.clone());

        // Create USB bus for TX
        let usb_bus = UsbMidiBus::new();

        (
            Shared {
                rx_bus: Bus::Uart(uart_bus),
                tx_bus: Bus::Usb(usb_bus),
            },
            Local {
                led: board::led(&mut gpio2, pins.p13),
                core: Core::new(midi_sender, glide_animator, envelope_animator),
                reset_btn,
            },
        )
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            cortex_m::asm::wfi();
        }
    }
    #[task(binds = LPUART6, shared = [rx_bus], priority = 2)]
    fn midi_handler(mut cx: midi_handler::Context) {
        cx.shared.rx_bus.lock(|bus| bus.poll());
    }

    #[task(shared = [tx_bus], priority = 2)]
    async fn midi_dispatch(mut cx: midi_dispatch::Context, mut r: MidiReceiver) -> ! {
        loop {
            let msg = r.recv().await.unwrap();
            cx.shared.tx_bus.lock(|bus| {
                if !matches!(
                    msg,
                    MidiMsg::ChannelVoice {
                        msg: midi_msg::ChannelVoiceMsg::PitchBend { .. },
                        ..
                    }
                ) {
                    info!(">>> {}", MidiFmt(&msg));
                }
                bus.send(&msg);
            });
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

    #[task(priority = 1)]
    #[allow(static_mut_refs)]
    async fn usb_host_init(_cx: usb_host_init::Context) {
        usbhost::init();
    }

    /// USB Host periodic maintenance task
    #[task(priority = 1)]
    async fn usb_host_maintenance(_cx: usb_host_maintenance::Context) {
        loop {
            usbhost::task();

            // Check connection status
            let connected = usbhost::midi_connected();

            if connected {
                let (vendor, product) = usbhost::get_device_info();
                info!(
                    "USB MIDI device CONNECTED - VID:{:04X} PID:{:04X}",
                    vendor, product
                );
                return;
            }
            // Sleep to avoid burning CPU - only need to service timers/timeouts
            Systick::delay(10.millis()).await;
        }
    }

    /// USB Host interrupt handler
    #[task(binds = USB_OTG2, shared = [tx_bus], priority = 2)]
    fn usb_host_isr(_cx: usb_host_isr::Context) {
        usbhost::usb_isr();
        if !usbhost::midi_connected() {
            // TODO: sucks, need to make a proper device state management later.
            // .ok() here is needed so the system avoids spawning another maintenance task while enumerating.
            usb_host_maintenance::spawn().ok();
        }
    }

    #[task(binds = USB_OTG1, priority = 2)]
    #[allow(static_mut_refs)]
    fn otg_interrupt(_cx: otg_interrupt::Context) {
        unsafe {
            crate::interrupt::free(|_| {
                if let Some(p) = POLLER.as_mut() {
                    p.poll();
                }
            });
        }
    }

    #[task(local = [led], priority = 1)]
    async fn blink_led(cx: blink_led::Context) {
        let led = cx.local.led;
        led.set();
        Systick::delay(1.millis()).await;
        led.clear();
    }

    /// Reset button interrupt handler
    ///
    /// P14 (GPIO1_IO18) with pulldown - wire button from P14 to 3.3V
    /// Triggers on rising edge, immediately resets the MCU
    #[task(binds = GPIO1_COMBINED_16_31, local = [reset_btn], priority = 2)]
    fn reset_button_isr(cx: reset_button_isr::Context) {
        let btn = cx.local.reset_btn;
        if btn.is_triggered() {
            btn.clear_triggered();
            cortex_m::peripheral::SCB::sys_reset();
        }
    }

    // ANIMATION BOILERPLATE
    #[task(priority = 2)]
    async fn animate_envelope(
        _cx: animate_envelope::Context,
        engine: AnimationEngine,
        sender: CoreSender,
    ) -> ! {
        run_animator_loop(engine, sender).await
    }

    #[task(priority = 2)]
    async fn animate_glide(
        _cx: animate_glide::Context,
        engine: AnimationEngine,
        sender: CoreSender,
    ) -> ! {
        run_animator_loop(engine, sender).await
    }
}
