#[rtic::app(device = teensy4_bsp, peripherals = true, dispatchers = [KPP, GPT1, TMR1])]
mod app {

    use crate::{
        app_event::{AppReceiver, AppSender},
        core::{AnimationEngine, Core, ANIMATORS_PER_VOICE, MAX_VOICES},
        midi::{MidiBus, MidiController},
        output::{OutputReceiver, OutputSender},
        runtime::RuntimeStorage,
        teensy::{
            SystickClock, UartMidiBus, UsbMidiBus,
            hardware::{handle_buttons_gpio3_interrupt, handle_buttons_gpio4_interrupt, handle_encoder_interrupt},
            midi::Bus,
        },
        ui::{ButtonReceiver, ButtonSender, EncoderDecoder, UiReader},
    };
    use board::t41 as brd;
    use embedded_alloc::LlffHeap as Heap;
    use imxrt_log as logging;
    use cortex_m::interrupt;
    use rtic_monotonics::systick::{Systick, *};
    use static_cell::StaticCell;
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
        pins::t41 as pins,
        ral::lpuart::Instance,
    };

    pub type MidiUart = Lpuart<Pins<pins::P20, pins::P21>, 8>;
    pub type Animator = crate::core::Animator;
    pub type CoreSender = AppSender;
    pub type CoreReceiver = AppReceiver;
    pub type MidiSender = OutputSender;
    pub type MidiReceiver = OutputReceiver;

    static STORAGE: StaticCell<RuntimeStorage> = StaticCell::new();

    const HEAP_SIZE: usize = 1024;
    const MIDI_BAUD: u32 = 31250;
    pub(crate) static mut POLLER: Option<logging::Poller> = None;

    #[shared]
    struct Shared {
        rx_bus: Bus, // UART input
    }

    #[local]
    struct Local {
        tx_bus: Bus, // USB output; owned by midi_dispatch
        led: board::Led,
        core: Core,
        reset_btn: gpio::Input<pins::P7>,
        button1: gpio::Input<pins::P28>,
        button2: gpio::Input<pins::P29>,
        button3: gpio::Input<pins::P30>,
        button4: gpio::Input<pins::P31>,
        buttons_gpio3_sender: ButtonSender,
        buttons_gpio4_sender: ButtonSender,
        adc1: adc::Adc<1>,
        adc2: adc::Adc<2>,
        slider0: adc::AnalogInput<pins::P24, 1>,
        slider1: adc::AnalogInput<pins::P25, 1>,
        slider2: adc::AnalogInput<pins::P26, 2>,
        slider3: adc::AnalogInput<pins::P27, 2>,
        enc_a: gpio::Input<pins::P39>,
        enc_b: gpio::Input<pins::P40>,
        enc_click: gpio::Input<pins::P41>,
        encoder: EncoderDecoder,
        enc_sender: CoreSender,
        enc_button_sender: ButtonSender,
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
            mut gpio3,
            mut gpio4,
            adc1,
            adc2,
            mut pins,
            usb,
            lpuart8,
            lpi2c1,
            ..
        } = brd(cx.device);

        let poller = logging::log::usbd(usb, logging::Interrupts::Enabled).unwrap();
        interrupt::free(|_| unsafe {
            POLLER = Some(poller);
        });
        log::set_max_level(configured_log_level());

        let handles = STORAGE.init(RuntimeStorage::new()).split();
        let mut output_sender = handles.output_sender;
        let core_sender = handles.app_sender;
        let button_sender = handles.button_sender;
        let midi_controller = MidiController::new();

        animate_voices::spawn(handles.engines, output_sender.clone()).unwrap();

        // The reset button connects P7 to ground, so use a pullup and falling-edge interrupt.
        iomuxc::configure(
            &mut pins.p7,
            iomuxc::Config::zero().set_pull_keeper(Some(iomuxc::PullKeeper::Pullup100k)),
        );
        let reset_btn = gpio2.input(pins.p7);
        gpio2.set_interrupt(&reset_btn, Some(gpio::Trigger::FallingEdge));

        // The encoder connects A/B/click to P39/P40/P41 and common/switch to ground.
        // The PCB supplies external pullups and debounce capacitors on all three signals.
        let enc_cfg = iomuxc::Config::zero().set_pull_keeper(Some(iomuxc::PullKeeper::Pullup100k));
        iomuxc::configure(&mut pins.p39, enc_cfg);
        iomuxc::configure(&mut pins.p40, enc_cfg);
        iomuxc::configure(&mut pins.p41, enc_cfg);
        let enc_a = gpio1.input(pins.p39);
        let enc_b = gpio1.input(pins.p40);
        let enc_click = gpio1.input(pins.p41);
        let encoder = EncoderDecoder::new(enc_a.is_set(), enc_b.is_set());
        gpio1.set_interrupt(&enc_a, Some(gpio::Trigger::EitherEdge));
        gpio1.set_interrupt(&enc_b, Some(gpio::Trigger::EitherEdge));
        gpio1.set_interrupt(&enc_click, Some(gpio::Trigger::EitherEdge));

        // P28-P31 are active-low buttons with external pullups and RC filtering.
        iomuxc::configure(&mut pins.p28, enc_cfg);
        iomuxc::configure(&mut pins.p29, enc_cfg);
        iomuxc::configure(&mut pins.p30, enc_cfg);
        iomuxc::configure(&mut pins.p31, enc_cfg);
        let button1 = gpio3.input(pins.p28);
        let button2 = gpio4.input(pins.p29);
        let button3 = gpio3.input(pins.p30);
        let button4 = gpio3.input(pins.p31);
        gpio3.set_interrupt(&button1, Some(gpio::Trigger::EitherEdge));
        gpio4.set_interrupt(&button2, Some(gpio::Trigger::EitherEdge));
        gpio3.set_interrupt(&button3, Some(gpio::Trigger::EitherEdge));
        gpio3.set_interrupt(&button4, Some(gpio::Trigger::EitherEdge));

        // PCB order is P24=Release, P25=Sustain, P26=Decay, P27=Attack.
        // P24/P25 are ADC1 inputs; P26/P27 are ADC2 inputs.
        // TODO(octaglide-task-25): move sliders to ACMP pins for interrupt-driven change detection.
        // IMXRT1060 has 4 ACMP channels (ACMP1-4) which would fit all 4 sliders.
        // Use tracking comparator pattern: read value in ISR, reprogram threshold to new_value ± hysteresis.
        let slider0 = adc::AnalogInput::new(pins.p24);
        let slider1 = adc::AnalogInput::new(pins.p25);
        let slider2 = adc::AnalogInput::new(pins.p26);
        let slider3 = adc::AnalogInput::new(pins.p27);

        let i2c = board::lpi2c(lpi2c1, pins.p19, pins.p18, board::Lpi2cClockSpeed::KHz400);
        oled_task::spawn(i2c, handles.ui_reader).ok();

        midi_dispatch::spawn(handles.output_receiver, midi_controller).unwrap();
        core_task::spawn(handles.app_receiver, output_sender.clone(), handles.ui_writer).unwrap();
        usb_host_maintenance::spawn(core_sender.clone()).unwrap();

        // Create UART bus for RX
        let uart_bus = UartMidiBus::new(
            prepare_uart(lpuart8, pins.p20, pins.p21),
            core_sender.clone(),
        );

        let enc_sender = core_sender.clone();
        let enc_button_sender = button_sender.clone();
        let buttons_gpio3_sender = button_sender.clone();
        let buttons_gpio4_sender = button_sender;
        button_task::spawn(handles.button_receiver, core_sender.clone()).unwrap();
        read_sliders::spawn(core_sender).unwrap();

        // Create USB bus for TX
        let usb_bus = UsbMidiBus::new();

        (
            Shared {
                rx_bus: Bus::Uart(uart_bus),
            },
            Local {
                tx_bus: Bus::Usb(usb_bus),
                led: board::led(&mut gpio2, pins.p13),
                core: Core::new(output_sender.clone(), handles.animators),
                reset_btn,
                button1,
                button2,
                button3,
                button4,
                buttons_gpio3_sender,
                buttons_gpio4_sender,
                adc1,
                adc2,
                slider0,
                slider1,
                slider2,
                slider3,
                enc_a,
                enc_b,
                enc_click,
                encoder,
                enc_sender,
                enc_button_sender,
            },
        )
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            cortex_m::asm::wfi();
        }
    }
    #[task(binds = LPUART8, shared = [rx_bus], priority = 4)]
    fn midi_handler(mut cx: midi_handler::Context) {
        cx.shared.rx_bus.lock(|bus| bus.poll());
    }

    /// Sole owner of the MIDI output bus. Runs above every application task so
    /// clock and notes are never delayed by UI work.
    #[task(local = [tx_bus], priority = 3)]
    async fn midi_dispatch(
        cx: midi_dispatch::Context,
        r: OutputReceiver,
        controller: MidiController,
    ) -> ! {
        crate::runtime::run_midi_dispatch(cx.local.tx_bus, r, controller).await
    }

    fn configured_log_level() -> log::LevelFilter {
        match env!("OCTAGLIDE_LOG") {
            "off" => log::LevelFilter::Off,
            "error" => log::LevelFilter::Error,
            "warn" => log::LevelFilter::Warn,
            "debug" => log::LevelFilter::Debug,
            "trace" => log::LevelFilter::Trace,
            _ => log::LevelFilter::Info,
        }
    }

    /// Owns the voice graph and the UI selection state.
    ///
    /// Priority 2 puts Core above the USB host maintenance task (1) and level
    /// with the UI tasks, while staying below `midi_dispatch` and
    /// `animate_voices` (3) so they still preempt and drain the output queue.
    ///
    /// The prio-2 deadlock recorded in the hang notes was `midi_dispatch` and
    /// `usb_host_isr` sharing this level; `usb_host_isr` is now priority 4, so
    /// Core at 2 cannot reproduce it.
    #[task(local = [core], priority = 2)]
    async fn core_task(
        cx: core_task::Context,
        receiver: CoreReceiver,
        output: OutputSender,
        ui_writer: crate::ui::UiWriter,
    ) -> ! {
        crate::runtime::run_core(cx.local.core, receiver, output, ui_writer).await
    }

    #[task(priority = 1)]
    async fn usb_host_maintenance(_cx: usb_host_maintenance::Context, sender: CoreSender) -> ! {
        crate::teensy::usb::run_host(sender).await
    }

    /// USB Host interrupt handler; maintenance is started once during init.
    ///
    /// Runs above every software task so it can always service transfer
    /// completions and timer events, including while `midi_dispatch` is
    /// waiting for TX buffer space. It must not declare `tx_bus`: doing so
    /// raises that resource's ceiling to this priority, which would mask this
    /// handler while the MIDI sender holds the lock.
    #[task(binds = USB_OTG2, priority = 4)]
    fn usb_host_isr(_cx: usb_host_isr::Context) {
        usbhost::usb_isr();
    }

    /// Logging poller. Deliberately lowest priority: it takes a global critical
    /// section, and diagnostics must never delay the I/O it is observing.
    #[task(binds = USB_OTG1, priority = 1)]
    #[allow(static_mut_refs)]
    fn otg_interrupt(_cx: otg_interrupt::Context) {
        unsafe {
            interrupt::free(|_| {
                if let Some(p) = POLLER.as_mut() {
                    p.poll();
                }
            });
        }
    }

    #[task(local = [led], priority = 2)]
    async fn blink_led(cx: blink_led::Context) {
        let led = cx.local.led;
        led.set();
        Systick::delay(1.millis()).await;
        led.clear();
    }

    /// Reset button interrupt handler
    ///
    /// P7 (GPIO2_IO17) is pulled high and grounded by the reset switch.
    /// A falling edge immediately resets the MCU.
    #[task(binds = GPIO2_COMBINED_16_31, local = [reset_btn], priority = 4)]
    fn reset_button_isr(cx: reset_button_isr::Context) {
        let btn = cx.local.reset_btn;
        if btn.is_triggered() {
            btn.clear_triggered();
            cortex_m::peripheral::SCB::sys_reset();
        }
    }

    /// Rotary encoder interrupt handler: P39 (A), P40 (B), P41 (Click).
    /// A/B use full quadrature decoding; switch edges share the button debounce task.
    #[task(
        binds = GPIO1_COMBINED_16_31,
        local = [enc_a, enc_b, enc_click, encoder, enc_sender, enc_button_sender],
        priority = 4
    )]
    fn encoder_isr(cx: encoder_isr::Context) {
        handle_encoder_interrupt(cx);
    }

    #[task(
        binds = GPIO3_COMBINED_16_31,
        local = [button1, button3, button4, buttons_gpio3_sender],
        priority = 4
    )]
    fn buttons_gpio3_isr(cx: buttons_gpio3_isr::Context) {
        handle_buttons_gpio3_interrupt(cx);
    }

    #[task(
        binds = GPIO4_COMBINED_16_31,
        local = [button2, buttons_gpio4_sender],
        priority = 4
    )]
    fn buttons_gpio4_isr(cx: buttons_gpio4_isr::Context) {
        handle_buttons_gpio4_interrupt(cx);
    }

    #[task(priority = 2)]
    async fn button_task(_cx: button_task::Context, receiver: ButtonReceiver, sender: CoreSender) -> ! {
        crate::runtime::run_buttons::<SystickClock>(receiver, sender).await
    }

    /// Periodically reads 4 ADC sliders and sends ADSR parameter updates to core.
    /// Sliders map: 0=Release, 1=Sustain, 2=Decay, 3=Attack.
    /// Only sends an update when a slider value changes by more than SLIDER_DEADBAND counts.
    #[task(local = [adc1, adc2, slider0, slider1, slider2, slider3, prev: [u16; 4] = [u16::MAX; 4]], priority = 2)]
    async fn read_sliders(cx: read_sliders::Context, sender: CoreSender) -> ! {
        crate::teensy::hardware::read_sliders(cx, sender).await
    }

    #[task(priority = 3)]
    async fn animate_voices(
        _cx: animate_voices::Context,
        engines: [[AnimationEngine; ANIMATORS_PER_VOICE]; MAX_VOICES],
        sender: MidiSender,
    ) {
        crate::runtime::run_animations::<SystickClock>(engines, sender).await
    }

    #[task(priority = 2)]
    async fn oled_task(_cx: oled_task::Context, i2c: board::Lpi2c1, updates: UiReader) {
        crate::teensy::run_envelope_display(i2c, updates).await;
    }
}
