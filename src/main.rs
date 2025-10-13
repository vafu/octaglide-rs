#![no_std]
#![no_main]

use teensy4_panic as _;

mod core;
mod midi;
mod processor;

#[macro_use]
extern crate alloc;

#[rtic::app(device = teensy4_bsp, peripherals = true, dispatchers = [KPP, GPT1])]
mod app {

    use crate::{
        core::{Core, Input as CoreIn, Output as CoreOut},
        midi::MidiBus,
    };
    use board::t40 as brd;
    use embedded_alloc::LlffHeap as Heap;
    use futures::FutureExt;
    use imxrt_log as logging;
    use log::info;
    use midi_msg::{Channel, ChannelVoiceMsg, ControlChange, MidiMsg};
    use rtic_monotonics::{
        Monotonic,
        systick::{Systick, *},
    };
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

    pub struct Dispatcher {
        midi_sender: Sender<'static, MidiMsg, MIDI_CHANNEL_CAPACITY>,
        slide_sender: Sender<'static, MidiMsg, 1>,
    }

    impl Dispatcher {
        pub async fn dispatch(&mut self, output: CoreOut) {
            match output {
                CoreOut::SendMidi(msg) => {
                    // TODO: Clone, meh
                    self.midi_sender.send(msg.clone()).await.unwrap();
                    self.slide_sender.send(msg).await.unwrap();
                }
                CoreOut::BlinkLed => {
                    blink_led::spawn().ok();
                }
                CoreOut::Slide(_) => {}
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
        let (s, r) = make_channel!(MidiMsg, MIDI_CHANNEL_CAPACITY);
        let (ss, sr) = make_channel!(MidiMsg, 1);

        let dispatcher = Dispatcher {
            midi_sender: s,
            slide_sender: ss,
        };

        midi_dispatch::spawn(r).unwrap();
        slide::spawn(sr).unwrap();

        (
            Shared {
                midi_bus: MidiBus::new(prepare_uart(lpuart6, pins.p1, pins.p0), |res| {
                    process_input::spawn(CoreIn::ProcessMidi(res)).unwrap();
                }),
            },
            Local {
                led: board::led(&mut gpio2, pins.p13),
                poller: logging::log::usbd(usb, logging::Interrupts::Enabled).unwrap(),
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
    async fn slide(mut ctx: slide::Context, mut r: Receiver<'static, MidiMsg, 1>) -> ! {
        let mut GLIDE_DURATION_MS: u32 = 100;
        const GLIDE_STEPS: u32 = 32;
        const PITCH_BEND_CENTER: u32 = 8192;

        let mut ch = Channel::Ch1;
        let mut start_time = Systick::now();
        let mut step = 0;

        'reset: loop {
            if step == 0 {
                match r.recv().await {
                    Ok(MidiMsg::ChannelVoice {
                        channel,
                        msg: ChannelVoiceMsg::NoteOn { .. },
                    }) => {
                        ch = channel;
                        start_time = Systick::now();
                        info!("start {}", start_time);
                    }
                    Ok(MidiMsg::ChannelVoice {
                        msg:
                            ChannelVoiceMsg::ControlChange {
                                control: ControlChange::CC { control: 13, value },
                            },
                        ..
                    }) => {
                        GLIDE_DURATION_MS = value as u32 * 10000 / 127;
                        continue 'reset;
                    }
                    _ => continue 'reset,
                }
            }

            if GLIDE_DURATION_MS == 0 {
                continue 'reset;
            }

            let next_step_time = start_time + ((step * GLIDE_DURATION_MS) / GLIDE_STEPS).millis();
            info!("nextstep {}", next_step_time);
            futures::select_biased! {
                                      msg = r.recv().fuse() => match  msg {
                                      Ok(MidiMsg::ChannelVoice { channel, msg: ChannelVoiceMsg::NoteOn {..}}) => {
                                          ch = channel;
                                          start_time = Systick::now();
                                          step = 0;
                                          continue 'reset;
                                      },
                        Ok(MidiMsg::ChannelVoice {
                                              msg:
                                                  ChannelVoiceMsg::ControlChange {
                                                      control: ControlChange::CC { control: 13, value },
                                                  },
                                              ..
                                          }) => {
            GLIDE_DURATION_MS = value as u32 * 10000 / 127;
                                    continue 'reset;

                                },
                                          _ => {}
                                      },

                                      _ = Systick::delay_until(next_step_time).fuse() => {
                                          if step == GLIDE_STEPS {
                                              step = 0;
                                              continue 'reset;
                                          }
                                          step += 1;
                                          let bend_value = ((step as u64 * PITCH_BEND_CENTER as u64) / GLIDE_STEPS as u64) as u16;
                                          let bend_msg = MidiMsg::ChannelVoice {
                                              channel: ch,
                                              msg: ChannelVoiceMsg::PitchBend { bend: bend_value },
                                          };
                                          ctx.shared.midi_bus.lock(|bus| bus.send(&bend_msg));
                                      }
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

    #[task(binds = USB_OTG1, local = [poller], priority = 2)]
    fn log_over_usb(cx: log_over_usb::Context) {
        cx.local.poller.poll();
    }
}
