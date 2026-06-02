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
mod state;
mod ui;
mod usb;

extern crate alloc;

#[rtic::app(device = teensy4_bsp, peripherals = true, dispatchers = [KPP, GPT1])]
mod app {

    use crate::{
        anim::animator::{AnimationEngine, Cmd},
        core::{Core, Input as CoreIn, MidiOut},
        midi::{Bus, MidiBus, MidiFmt, UartMidiBus},
        usb::UsbMidiBus,
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
            adc, gpio,
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
    pub type MidiSender = Sender<'static, MidiOut, MIDI_CHANNEL_CAPACITY>;
    pub type MidiReceiver = Receiver<'static, MidiOut, MIDI_CHANNEL_CAPACITY>;
    pub type Animator = Sender<'static, Cmd, 1>;
    pub type CoreSender = Sender<'static, CoreIn, CORE_INPUT_CHANNEL_CAPACITY>;
    pub type CoreReceiver = Receiver<'static, CoreIn, CORE_INPUT_CHANNEL_CAPACITY>;

    macro_rules! make_animator {
        ($task_name:ident, $midi_sender:expr) => {{
            let (animator, cmd_rx) = make_channel!(Cmd, 1);
            let engine = AnimationEngine::new(cmd_rx);
            $task_name::spawn(engine, $midi_sender.clone()).unwrap();
            animator
        }};
    }

    async fn run_animator_loop(
        mut engine: AnimationEngine,
        mut sender: MidiSender,
        tag: &'static str,
    ) -> ! {
        loop {
            if let Some(msgs) = engine.tick().await {
                for msg in msgs {
                    sender.send(MidiOut { msg, tag }).await.ok();
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
        reset_btn: gpio::Input<pins::P33>,
        adc1: adc::Adc<1>,
        slider0: adc::AnalogInput<pins::P14, 1>,
        slider1: adc::AnalogInput<pins::P15, 1>,
        slider2: adc::AnalogInput<pins::P24, 1>,
        slider3: adc::AnalogInput<pins::P25, 1>,
        enc_a: gpio::Input<pins::P20>,
        enc_b: gpio::Input<pins::P19>,
        enc_click: gpio::Input<pins::P18>,
        enc_sender: CoreSender,
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
            mut gpio1,
            mut gpio2,
            mut gpio4,
            adc1,
            mut pins,
            usb,
            lpuart6,
            lpi2c3,
            ..
        } = brd(cx.device);

        let poller = logging::log::usbd(usb, logging::Interrupts::Enabled).unwrap();
        crate::interrupt::free(|_| unsafe {
            POLLER = Some(poller);
        });

        let (midi_sender, midi_receiver) = make_channel!(MidiOut, MIDI_CHANNEL_CAPACITY);
        let (core_sender, core_receiver) = make_channel!(CoreIn, CORE_INPUT_CHANNEL_CAPACITY);

        let glide_animator = make_animator!(animate_glide, midi_sender);
        let envelope_animator = make_animator!(animate_envelope, midi_sender);

        // Configure reset button (P33 / GPIO4_IO07) with pullup for interrupt on rising edge
        // Wire momentary button from P33 to 3.3V
        iomuxc::configure(
            &mut pins.p33,
            iomuxc::Config::zero().set_pull_keeper(Some(iomuxc::PullKeeper::Pullup100k)),
        );
        let reset_btn = gpio4.input(pins.p33);

        // Enable GPIO interrupt on rising edge (button press)
        gpio4.set_interrupt(&reset_btn, Some(gpio::Trigger::RisingEdge));

        // Configure rotary encoder on P20 (A), P19 (B), P18 (Click) — all GPIO1_COMBINED_16_31
        // Wiring: EC11 common → GND, A/B/Click → internal 100kΩ pullup, falling edge
        // TODO(task-28): upgrade to full quadrature decoding for better reliability at high speed
        let enc_cfg = iomuxc::Config::zero().set_pull_keeper(Some(iomuxc::PullKeeper::Pullup100k));
        iomuxc::configure(&mut pins.p18, enc_cfg);
        iomuxc::configure(&mut pins.p19, enc_cfg);
        iomuxc::configure(&mut pins.p20, enc_cfg);
        let enc_a = gpio1.input(pins.p20);
        let enc_b = gpio1.input(pins.p19);
        let enc_click = gpio1.input(pins.p18);
        gpio1.set_interrupt(&enc_a, Some(gpio::Trigger::FallingEdge));
        gpio1.set_interrupt(&enc_click, Some(gpio::Trigger::FallingEdge));

        // Configure ADC sliders: Attack/Decay on P14/P15, Sustain/Release moved to P24/P25
        // (P16/P17 freed for LPI2C3 OLED)
        // TODO: move sliders to ACMP (comparator) pins for interrupt-driven change detection.
        // IMXRT1060 has 4 ACMP channels (ACMP1-4) which would fit all 4 sliders.
        // Use tracking comparator pattern: read value in ISR, reprogram threshold to new_value ± hysteresis.
        let slider0 = adc::AnalogInput::new(pins.p14);
        let slider1 = adc::AnalogInput::new(pins.p15);
        let slider2 = adc::AnalogInput::new(pins.p24);
        let slider3 = adc::AnalogInput::new(pins.p25);

        let i2c = board::lpi2c(lpi2c3, pins.p16, pins.p17, board::Lpi2cClockSpeed::MHz1);
        oled_task::spawn(i2c).ok();

        midi_dispatch::spawn(midi_receiver).unwrap();
        core_task::spawn(core_receiver).unwrap();
        usb_host_init::spawn().unwrap();

        // Create UART bus for RX
        let uart_bus =
            UartMidiBus::new(prepare_uart(lpuart6, pins.p1, pins.p0), core_sender.clone());

        let enc_sender = core_sender.clone();
        read_sliders::spawn(core_sender).unwrap();

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
                adc1,
                slider0,
                slider1,
                slider2,
                slider3,
                enc_a,
                enc_b,
                enc_click,
                enc_sender,
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
            let MidiOut { msg, tag } = r.recv().await.unwrap();
            cx.shared.tx_bus.lock(|bus| {
                if !matches!(
                    msg,
                    MidiMsg::ChannelVoice {
                        msg: midi_msg::ChannelVoiceMsg::PitchBend { .. },
                        ..
                    }
                ) {
                    info!(">>> [{}] {}", tag, MidiFmt(&msg));
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
    /// P33 (GPIO4_IO07) with pullup - wire button from P33 to 3.3V
    /// Triggers on rising edge, immediately resets the MCU
    #[task(binds = GPIO4_COMBINED_0_15, local = [reset_btn], priority = 2)]
    fn reset_button_isr(cx: reset_button_isr::Context) {
        let btn = cx.local.reset_btn;
        if btn.is_triggered() {
            btn.clear_triggered();
            cortex_m::peripheral::SCB::sys_reset();
        }
    }

    /// Rotary encoder interrupt handler — P18 (A), P19 (B), P20 (Click), all GPIO1_COMBINED_16_31.
    /// Simple decoding: interrupt on A rising edge, read B level for direction.
    /// TODO(task-28): upgrade to full quadrature decoding (both edges of A+B, lookup table)
    ///                for better reliability at high speed and natural bounce filtering.
    #[task(binds = GPIO1_COMBINED_16_31, local = [enc_a, enc_b, enc_click, enc_sender], priority = 2)]
    fn encoder_isr(cx: encoder_isr::Context) {
        crate::ui::handle_encoder_interrupt(cx);
    }

    /// Periodically reads 4 ADC sliders and sends ADSR parameter updates to core.
    /// Sliders map: 0=Attack, 1=Decay, 2=Sustain, 3=Release
    /// Only sends an update when a slider value changes by more than SLIDER_DEADBAND counts.
    #[task(local = [adc1, slider0, slider1, slider2, slider3, prev: [u16; 4] = [u16::MAX; 4]], priority = 1)]
    async fn read_sliders(cx: read_sliders::Context, sender: CoreSender) -> ! {
        crate::ui::read_sliders(cx, sender).await
    }

    // ANIMATION BOILERPLATE
    #[task(priority = 2)]
    async fn animate_envelope(
        _cx: animate_envelope::Context,
        engine: AnimationEngine,
        sender: MidiSender,
    ) -> ! {
        run_animator_loop(engine, sender, "env").await
    }

    #[task(priority = 2)]
    async fn animate_glide(
        _cx: animate_glide::Context,
        engine: AnimationEngine,
        sender: MidiSender,
    ) -> ! {
        run_animator_loop(engine, sender, "glide").await
    }

    #[task(priority = 1)]
    async fn oled_task(_cx: oled_task::Context, i2c: board::Lpi2c3) {
        crate::ui::run_envelope_display(i2c).await;
    }
}
