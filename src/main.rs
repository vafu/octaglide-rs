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

use rtic_monotonics::{Monotonic, systick::Systick};

#[rtic::app(device = teensy4_bsp, peripherals = true, dispatchers = [KPP, GPT1])]
mod app {

    use crate::{
        anim::animator::{Animator, Cmd},
        core::{Core, Input as CoreIn, MidiEvent},
        midi::MidiBus,
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
            gpio::Port,
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
        poller: logging::Poller,
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
            mut gpio3,
            mut pins,
            usb,
            lpuart6,
            ..
        } = brd(cx.device);


        const PIN_CONFIG: iomuxc::Config =
            iomuxc::Config::zero().set_drive_strength(iomuxc::DriveStrength::R0_7);
        iomuxc::configure(&mut pins.p28, PIN_CONFIG);
        let output = gpio3.output(pins.p28);
        output.set();

        init_heap();
        Systick::start(
            cx.core.SYST,
            board::ARM_FREQUENCY,
            rtic_monotonics::create_systick_token!(),
        );

        let poller = logging::log::usbd(usb, logging::Interrupts::Enabled).unwrap();

        let (midi_sender, midi_receiver) = make_channel!(MidiMsg, MIDI_CHANNEL_CAPACITY);
        let (animator, animator_sender) = Animator::new();
        let (core_sender, core_receiver) = make_channel!(CoreIn, CORE_INPUT_CHANNEL_CAPACITY);

        midi_dispatch::spawn(midi_receiver).unwrap();
        animate::spawn(animator, core_sender.clone()).unwrap();
        core_task::spawn(core_receiver).ok();
        usb_host_init::spawn(core_sender.clone()).ok();

        let led = board::led(&mut gpio2, pins.p13);
        (
            Shared {
                midi_bus: MidiBus::new(
                    prepare_uart(lpuart6, pins.p1, pins.p0),
                    core_sender.clone(),
                ),
            },
            Local {
                led,
                core: Core::new(midi_sender, animator_sender),
                poller,
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
        Systick::delay(100.millis()).await;
        led.clear();
    }

    #[task(binds = USB_OTG1, local = [poller], priority = 2)]
    #[allow(static_mut_refs)]
    fn log_over_usb(cx: log_over_usb::Context) {
        cx.local.poller.poll();
    }

    #[task(binds = USB_OTG2, priority = 2)]
    fn usb_host_isr(_cx: usb_host_isr::Context) {
        unsafe {
            usbhost::usb_isr();
        }
    }

    #[task(priority = 1)]
    #[allow(static_mut_refs)]
    async fn usb_host_init(_cx: usb_host_init::Context, sender: CoreSender) {
        info!("usb_host_init: waiting 3 seconds for USB logging to be ready...");
        Systick::delay(3000.millis()).await;

        // Set up time source for USB host library
        usbhost::set_time_source(|| {
            use rtic_monotonics::Monotonic;
            Systick::now().duration_since_epoch().to_micros()
        });

        unsafe {
            usbhost::init();
        };
        usb_host_test::spawn(sender).ok();
    }

    #[task(priority = 2)]
    async fn usb_host_test(_cx: usb_host_test::Context, _sender: CoreSender) -> ! {
        info!("usb_host_test: starting USB MIDI output test loop");

        let mut note = 60u8; // Start at middle C
        let channel = 1; // MIDI channel 1 (0-indexed)
        let velocity = 100;
        let mut last_connected = false;
        let mut note_timer = 0u32; // Count poll cycles for note timing

        loop {
            // CRITICAL: Must call task() every iteration to process USB transfers
            unsafe {
                usbhost::task();
            }

            // Check connection status
            let connected = unsafe { usbhost::midi_connected() };

            if connected != last_connected {
                if connected {
                    let (vendor, product) = unsafe { usbhost::get_device_info() };
                    info!(
                        "USB MIDI device CONNECTED - VID:{:04X} PID:{:04X}",
                        vendor, product
                    );
                    note_timer = 0; // Reset timer on connect
                } else {
                    info!("USB MIDI device DISCONNECTED");
                }
                last_connected = connected;
            }

            // Only send notes if device is connected
            if connected {
                note_timer += 1;

                // Send note every 100 polls (~1 second at 10ms intervals)
                if note_timer == 1 {
                    unsafe {
                        info!(
                            "Sending USB MIDI: NoteOn  note={} vel={} ch={}",
                            note, velocity, channel
                        );
                        usbhost::send_note_on(note, velocity, channel);
                    }
                } else if note_timer == 50 {
                    unsafe {
                        info!(
                            "Sending USB MIDI: NoteOff note={} vel={} ch={}",
                            note, velocity, channel
                        );
                        usbhost::send_note_off(note, velocity, channel);
                    }
                } else if note_timer >= 100 {
                    // Move to next note (cycle through C major scale)
                    note = match note {
                        60 => 62, // C -> D
                        62 => 64, // D -> E
                        64 => 65, // E -> F
                        65 => 67, // F -> G
                        67 => 69, // G -> A
                        69 => 71, // A -> B
                        71 => 72, // B -> C
                        _ => 60,  // C (octave up) -> back to C
                    };
                    note_timer = 0; // Reset for next note
                }
            }
            // Yield to other tasks occasionally (Task() needs to run frequently for USB transfers)
            if (note_timer % 10) == 0 {
                Systick::delay(10.millis()).await;
            }
        }
    }
}
