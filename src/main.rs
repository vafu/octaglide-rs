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

// Link to C++
unsafe extern "C" {
    fn cpp_init();
    fn cpp_task();
    fn cpp_usb_isr();
    fn cpp_send_note_on(note: u8, velocity: u8, channel: u8);
    fn cpp_send_note_off(note: u8, velocity: u8, channel: u8);
    fn cpp_midi_connected() -> i32;
    fn cpp_midi_get_device_info(vendor: *mut u16, product: *mut u16);
    fn cpp_debug_status();
    fn cpp_recheck_power();
    fn cpp_verify_clocks();
    fn cpp_configure_usb_power();
}

// Expose Rust time functions to C++
use rtic_monotonics::{
    Monotonic,
    systick::{Systick, fugit},
};

#[unsafe(no_mangle)]
pub extern "C" fn rust_micros() -> u32 {
    Systick::now().duration_since_epoch().to_micros()
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_log_info(msg: *const u8) {
    use ::core::{slice, str};
    if !msg.is_null() {
        unsafe {
            let mut len = 0;
            while *msg.add(len) != 0 {
                len += 1;
            }
            if let Ok(s) = str::from_utf8(slice::from_raw_parts(msg, len)) {
                log::info!("[C++] {}", s);
            }
        }
    }
}

// Parse USB MIDI message from USBHost_t36 format
// The USBHost library returns: type (status), data1, data2
fn parse_usb_midi(msg_type: u8, data1: u8, data2: u8) -> Option<midi_msg::MidiMsg> {
    use midi_msg::{Channel, ChannelVoiceMsg, ControlChange, MidiMsg};

    let status = msg_type & 0xF0;
    let channel = Channel::from_u8(msg_type & 0x0F);

    match status {
        0x80 => Some(MidiMsg::ChannelVoice {
            channel,
            msg: ChannelVoiceMsg::NoteOff {
                note: data1,
                velocity: data2,
            },
        }),
        0x90 => Some(MidiMsg::ChannelVoice {
            channel,
            msg: ChannelVoiceMsg::NoteOn {
                note: data1,
                velocity: data2,
            },
        }),
        0xB0 => Some(MidiMsg::ChannelVoice {
            channel,
            msg: ChannelVoiceMsg::ControlChange {
                control: ControlChange::CC {
                    control: data1,
                    value: data2,
                },
            },
        }),
        0xE0 => {
            // Pitch bend: combine data1 (LSB) and data2 (MSB) into 14-bit value
            let bend_value = ((data2 as u16) << 7) | (data1 as u16);
            Some(MidiMsg::ChannelVoice {
                channel,
                msg: ChannelVoiceMsg::PitchBend { bend: bend_value },
            })
        }
        _ => {
            log::warn!("Unsupported USB MIDI type: {:02X}", msg_type);
            None
        }
    }
}

#[rtic::app(device = teensy4_bsp, peripherals = true, dispatchers = [KPP, GPT1])]
mod app {

    use crate::{
        anim::animator::{Animator, Cmd},
        core::{Core, Input as CoreIn, MidiEvent},
        cpp_configure_usb_power, cpp_debug_status, cpp_init, cpp_midi_connected,
        cpp_midi_get_device_info, cpp_recheck_power, cpp_send_note_off, cpp_send_note_on, cpp_task,
        cpp_usb_isr, cpp_verify_clocks,
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
        let mut instances: board::Instances = cx.device.into();

        let board::Resources {
            mut gpio2,
            mut gpio3,
            mut pins,
            usb,
            lpuart6,
            ..
        } = brd(instances);

        let mut core = cx.core;
        let mut led = board::led(&mut gpio2, pins.p13);

        const PIN_CONFIG: iomuxc::Config =
            iomuxc::Config::zero()
            .set_drive_strength(iomuxc::DriveStrength::R0_7);
        iomuxc::configure(&mut pins.p28, PIN_CONFIG);
        let output = gpio3.output(pins.p28);
        output.set();

        init_heap();
        Systick::start(
            core.SYST,
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
        usb_host_init::spawn(core_sender.clone()).ok();

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

    #[task(binds = USB_OTG2, priority = 2)]
    fn usb_host_isr(_cx: usb_host_isr::Context) {
        unsafe {
            cpp_usb_isr();
        }
    }

    #[task(priority = 1)]
    #[allow(static_mut_refs)]
    async fn usb_host_init(_cx: usb_host_init::Context, sender: CoreSender) {
        info!("usb_host_init: waiting 3 seconds for USB logging to be ready...");
        Systick::delay(3000.millis()).await;

        unsafe {
            cpp_init();
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
        let mut debug_timer = 0u32; // For periodic debug output
        let mut gpio_check_timer = 0u32; // For GPIO verification

        loop {
            unsafe {
                cpp_task(); // Drive the C++ USBHost state machine
            }

            // Check connection status
            let connected = unsafe { cpp_midi_connected() == 1 };

            if connected != last_connected {
                if connected {
                    let mut vendor: u16 = 0;
                    let mut product: u16 = 0;
                    unsafe {
                        cpp_midi_get_device_info(&mut vendor, &mut product);
                    }
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
                        cpp_send_note_on(note, velocity, channel);
                    }
                } else if note_timer == 50 {
                    unsafe {
                        info!(
                            "Sending USB MIDI: NoteOff note={} vel={} ch={}",
                            note, velocity, channel
                        );
                        cpp_send_note_off(note, velocity, channel);
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

            // Call Task() as fast as possible - no delay
            // Yield to other tasks occasionally
            if (note_timer % 1000) == 0 {
                Systick::delay(500.millis()).await;
            }
        }
    }
}
