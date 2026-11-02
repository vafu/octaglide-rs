use super::{
    Input, OutputEvents,
    consumers::{Consumer, Glider, ModTrigger, SliderCatchUp, SliderTarget},
    *,
};
use crate::{
    anim::{
        Cmd, Settings,
        modulators::{Envelope, Glide, Modulation, Modulator},
    },
    notes,
    ui::{DisplaySnapshot, UiModel, UiProvider, UiWriter},
};
use futures::executor::block_on;
use midi_msg::{ChannelVoiceMsg, ControlChange};
use rtic_sync::{
    channel::{Channel as Queue, Receiver, Sender},
    signal::Signal,
};
use std::{boxed::Box, sync::Mutex};

// Production notes are shared by design. Serialize cases that access them.
static TEST_NOTES: Mutex<()> = Mutex::new(());

fn channel<T, const N: usize>() -> (Sender<'static, T, N>, Receiver<'static, T, N>) {
    Box::leak(Box::new(Queue::<T, N>::new())).split()
}

fn ui_channel() -> (
    UiWriter,
    rtic_sync::signal::SignalReader<'static, DisplaySnapshot>,
) {
    Box::leak(Box::new(Signal::new())).split()
}

fn read_ui(
    reader: &mut rtic_sync::signal::SignalReader<'static, DisplaySnapshot>,
) -> Option<Option<UiModel>> {
    reader.try_read().map(|snapshot| snapshot.model)
}

// Existing cases exercise voice zero. Still construct the production four-voice
// configuration; unused command receivers are intentionally disconnected.
fn test_core(
    midi: crate::app::MidiSender,
    glide: crate::app::Animator,
    first: crate::app::Animator,
    writer: UiWriter,
) -> Core {
    let mut resources = ::core::array::from_fn(|_| {
        let (glide, _) = channel();
        let (first, _) = channel();
        let (second, _) = channel();
        crate::anim::VoiceAnimators {
            glide,
            envelopes: [first, second],
        }
    });
    resources[0].glide = glide;
    resources[0].envelopes[0] = first;
    Core::new(midi, resources, writer)
}

fn slider(index: u8, value: u16) -> Input {
    Input::Slider {
        index,
        value,
        alternate: false,
    }
}

fn control_value(value: u8) -> u16 {
    (value as u32 * u16::MAX as u32).div_ceil(127) as u16
}

// Move to the value actually shown on the selected envelope before editing it.
async fn pickup_slider(core: &mut Core, index: u8) {
    let Some(UiModel::ModTrigger(model)) = core.ui_snapshot().model else {
        panic!("selected envelope missing")
    };
    let (_, alternate) = core.ui_consumer.slider_context();
    let value = match (alternate, index) {
        (false, 3) => model.attack.duration,
        (false, 2) => model.decay.duration,
        (_, 1) => model.sustain,
        (false, 0) => model.release.duration,
        (true, 3) => model.attack.curve,
        (true, 2) => model.decay.curve,
        (true, 0) => model.release.curve,
        _ => panic!("invalid slider"),
    };
    core.process(slider(index, control_value(value))).await;
}

fn turn(increment: bool) -> Input {
    Input::Encoder { increment }
}

async fn select_by_button(core: &mut Core, index: u8) {
    core.process(Input::EncoderClick { pressed: true }).await;
    core.process(Input::Button {
        index,
        pressed: true,
    })
    .await;
    core.process(Input::Button {
        index,
        pressed: false,
    })
    .await;
    core.process(Input::EncoderClick { pressed: false }).await;
}

async fn select_lane_by_button(core: &mut Core, index: u8) {
    core.process(Input::Button {
        index,
        pressed: true,
    })
    .await;
    core.process(Input::Button {
        index,
        pressed: false,
    })
    .await;
}

fn note_on() -> Input {
    Input::Midi(midi_note(Channel::Ch1, 100))
}

fn midi_note(channel: Channel, velocity: u8) -> MidiMsg {
    MidiMsg::ChannelVoice {
        channel,
        msg: ChannelVoiceMsg::NoteOn { note: 60, velocity },
    }
}

async fn clear_notes(voice: u8) {
    notes::edit_notes(VoiceId(voice), |notes| {
        notes.notes.clear();
        notes.bits.fill(0);
    })
    .await
    .unwrap();
}

#[test]
fn settings_are_owned_per_consumer_and_start_uses_constructor_routing() {
    block_on(async {
        let (first_tx, mut first_rx) = channel();
        let (second_tx, _) = channel();
        let mut first = ModTrigger::new(VoiceId(0), Channel::Ch5, 11, first_tx);
        let second = ModTrigger::new(VoiceId(0), Channel::Ch6, 12, second_tx);
        let mut out = OutputEvents::new();
        assert!(first.consume(&slider(3, u16::MAX), &mut out).await);
        assert!(out.is_empty());
        assert_eq!(first.ui_snapshot().attack.duration, 127);
        assert_eq!(second.ui_snapshot().attack.duration, 20);
        let Cmd::Update(Settings::Envelope(settings)) = first_rx.recv().await.unwrap() else {
            panic!("missing live settings update")
        };
        assert_eq!(settings.attack.duration, 127);
        assert!(!first.consume(&note_on(), &mut out).await);
        assert!(out.is_empty()); // Voice, rather than the consumer, forwards this note.
        let Cmd::Start(Modulator::Envelope(mut envelope)) = first_rx.recv().await.unwrap() else {
            panic!("missing envelope start")
        };
        assert_eq!(envelope.duration_ms().await, 127 * 16);
        assert!(matches!(
            envelope.animate(1.0, 1.0, 0.0).await.unwrap()[0],
            MidiMsg::ChannelVoice {
                channel: Channel::Ch5,
                msg: ChannelVoiceMsg::ControlChange {
                    control: ControlChange::CC {
                        control: 11,
                        value: 127
                    }
                },
            }
        ));
    });
}

#[test]
fn disabling_glide_keeps_constructor_bound_note_tracking_active() {
    let _guard = TEST_NOTES.lock().unwrap();
    block_on(async {
        clear_notes(0).await;
        clear_notes(1).await;
        let (sender, mut commands) = channel();
        let mut glider = Glider::new(VoiceId(1), Channel::Ch5, sender);
        let mut out = OutputEvents::new();
        glider.consume(&slider(0, 0), &mut out).await;
        assert!(!glider.ui_snapshot().enabled);
        assert!(matches!(
            commands.recv().await.unwrap(),
            Cmd::Update(Settings::Glide(_))
        ));
        assert!(!glider.consume(&note_on(), &mut out).await);
        assert!(out.is_empty()); // Voice, rather than the consumer, forwards this note.
        assert_eq!(
            notes::read_notes(VoiceId(1), |notes| notes.is_held(60)).await,
            Some(true)
        );
        assert_eq!(
            notes::read_notes(VoiceId(0), |notes| notes.any_held()).await,
            Some(false)
        );
        glider
            .consume(
                &Input::Midi(MidiMsg::ChannelVoice {
                    channel: Channel::Ch1,
                    msg: ChannelVoiceMsg::NoteOff {
                        note: 60,
                        velocity: 0,
                    },
                }),
                &mut out,
            )
            .await;
        assert_eq!(
            notes::read_notes(VoiceId(1), |notes| notes.any_held()).await,
            Some(false)
        );
    });
}

#[test]
fn animations_read_live_shared_notes_and_accept_live_settings() {
    let _guard = TEST_NOTES.lock().unwrap();
    block_on(async {
        clear_notes(0).await;
        clear_notes(1).await;
        notes::edit_notes(VoiceId(0), |notes| notes.press(60))
            .await
            .unwrap();
        assert_eq!(
            notes::read_notes(VoiceId(1), |notes| notes.any_held()).await,
            Some(false)
        );
        assert!(notes::read_notes(VoiceId(99), |_| ()).await.is_none());

        let mut glide = Glide::new(VoiceId(0), Channel::Ch5, GlideState::INITIAL, 60, 64);
        let messages = glide.animate(1.0, 1.0, 0.0).await.unwrap();
        assert!(!messages.iter().any(|msg| matches!(
            msg,
            MidiMsg::ChannelVoice {
                msg: ChannelVoiceMsg::NoteOff { note: 60, .. },
                ..
            }
        )));
        notes::edit_notes(VoiceId(0), |notes| notes.release(60))
            .await
            .unwrap();
        let mut glide = Glide::new(VoiceId(0), Channel::Ch5, GlideState::INITIAL, 60, 64);
        let messages = glide.animate(1.0, 1.0, 0.0).await.unwrap();
        assert!(messages.iter().any(|msg| matches!(
            msg,
            MidiMsg::ChannelVoice {
                msg: ChannelVoiceMsg::NoteOff { note: 60, .. },
                ..
            }
        )));
        notes::edit_notes(VoiceId(0), |notes| notes.press(64))
            .await
            .unwrap();
        let reset = glide.reset().await.unwrap();
        assert!(!reset.iter().any(|msg| matches!(
            msg,
            MidiMsg::ChannelVoice {
                msg: ChannelVoiceMsg::NoteOff { note: 64, .. },
                ..
            }
        )));
        let mut settings = GlideState::INITIAL;
        settings.duration_ms = 999;
        glide.update(&Settings::Glide(settings));
        assert_eq!(glide.duration_ms().await, 999);

        let mut settings = EnvelopeState::INITIAL;
        settings.mode = 1; // AR: ramp, hold, ramp.
        let mut envelope = Envelope::new(VoiceId(0), Channel::Ch5, 7, settings);
        assert!(envelope.next_stage().await);
        assert_eq!(envelope.duration_ms().await, u32::MAX);
        notes::edit_notes(VoiceId(0), |notes| notes.release(64))
            .await
            .unwrap();
        assert_eq!(envelope.duration_ms().await, 0);
        assert!(envelope.next_stage().await); // Release is index 2.
        envelope.update(&Settings::Envelope(EnvelopeState::INITIAL)); // AD has only 2 stages.
        assert_eq!(envelope.duration_ms().await, 20 * 16);
        assert!(envelope.animate(0.0, 1.0, 0.0).await.is_some());
    });
}

#[test]
fn core_publishes_only_changed_snapshots_and_rejects_missing_consumers() {
    block_on(async {
        let (midi, _) = channel();
        let (glide, _) = channel();
        let (envelope, mut commands) = channel();
        let (writer, mut reader) = ui_channel();
        let mut core = test_core(midi, glide, envelope, writer);
        let Some(Some(UiModel::ModTrigger(initial))) = read_ui(&mut reader) else {
            panic!("missing initial UI")
        };
        assert_eq!(initial.attack.duration, 20);
        pickup_slider(&mut core, 3).await;
        core.process(slider(3, u16::MAX)).await;
        let Some(Some(UiModel::ModTrigger(changed))) = read_ui(&mut reader) else {
            panic!("missing changed UI")
        };
        assert_eq!(changed.attack.duration, 127);
        let _ = commands.recv().await.unwrap();
        core.process(slider(3, u16::MAX)).await;
        assert!(read_ui(&mut reader).is_none());
        core.process(slider(3, 0)).await;
        let _ = commands.recv().await.unwrap();
        core.process(slider(3, u16::MAX)).await;
        let _ = commands.recv().await.unwrap();
        let Some(Some(UiModel::ModTrigger(latest))) = read_ui(&mut reader) else {
            panic!("missing latest UI")
        };
        assert_eq!(latest.attack.duration, 127);
        assert!(!core.select(TrackId(99), VoiceId(0), 0));
        assert!(read_ui(&mut reader).is_none());
        assert!(!core.select(TrackId(0), VoiceId(0), 4));
        assert!(read_ui(&mut reader).is_none());
        assert!(core.select(TrackId(0), VoiceId(0), 2));
        assert!(read_ui(&mut reader).is_none());
    });
}

#[test]
fn hardware_only_reaches_selected_lane_and_preserves_envelope_surface() {
    block_on(async {
        let (midi, mut sent) = channel();
        let (glide, _) = channel();
        let (envelope, mut commands) = channel();
        let (writer, mut reader) = ui_channel();
        let mut core = test_core(midi, glide, envelope, writer);
        let _ = read_ui(&mut reader);
        core.process(Input::EncoderClick { pressed: true }).await;
        core.process(Input::EncoderClick { pressed: false }).await;
        assert!(read_ui(&mut reader).is_none()); // A plain click changes nothing.
        core.process(Input::EncoderClick { pressed: true }).await;
        pickup_slider(&mut core, 3).await;
        core.process(slider(3, u16::MAX)).await;
        let Some(Some(UiModel::ModTrigger(curve))) = read_ui(&mut reader) else {
            panic!("missing curve update")
        };
        assert_eq!(curve.attack.curve, 127);
        assert_eq!(curve.attack.duration, 20);
        let _ = commands.recv().await.unwrap();
        core.process(Input::EncoderClick { pressed: false }).await;
        assert!(core.select(TrackId(0), VoiceId(0), 3));
        let _ = read_ui(&mut reader);
        core.process(turn(true)).await;
        let Some(Some(UiModel::ModTrigger(selected))) = read_ui(&mut reader) else {
            panic!("encoder must reach the selected second envelope")
        };
        assert_eq!(core.selected_track, TrackId(0));
        assert_eq!(core.selected_voice, VoiceId(0));
        assert_eq!(selected.mode, 1);
        let Some(UiModel::ModTrigger(envelope)) = core.voices[0].modulations[2].ui_snapshot()
        else {
            panic!("missing envelope model")
        };
        assert_eq!(envelope.mode, 0); // Encoder was not broadcast to the envelope.
        assert_eq!(envelope.attack.curve, 127);
        assert!(commands.try_recv().is_err());
        assert!(sent.try_recv().is_err()); // Hardware actions emit no MIDI.
    });
}

#[test]
fn two_envelopes_forward_notes_but_only_selected_one_receives_controls() {
    let _guard = TEST_NOTES.lock().unwrap();
    block_on(async {
        clear_notes(0).await;
        let (midi, mut sent) = channel();
        let (glide, _) = channel();
        let (first, mut first_commands) = channel();
        let (second, mut second_commands) = channel();
        let (writer, _) = ui_channel();
        let mut core = test_core(midi, glide, first, writer);
        core.voices[0].modulations[3] = Consumers::ModTrigger(SliderCatchUp::new(ModTrigger::new(
            VoiceId(0),
            Channel::Ch5,
            12,
            second,
        )));
        assert!(core.select(TrackId(0), VoiceId(0), 3));
        pickup_slider(&mut core, 3).await;
        core.process(slider(3, u16::MAX)).await;
        assert!(first_commands.try_recv().is_err());
        assert!(matches!(
            second_commands.recv().await.unwrap(),
            Cmd::Update(Settings::Envelope(_))
        ));
        let Some(UiModel::ModTrigger(first_ui)) = core.voices[0].modulations[2].ui_snapshot()
        else {
            panic!("missing first envelope")
        };
        assert_eq!(first_ui.attack.duration, 20);
        core.process(Input::Midi(midi_note(Channel::Ch1, 100)))
            .await;
        let Cmd::Start(Modulator::Envelope(first)) = first_commands.recv().await.unwrap() else {
            panic!("missing first start")
        };
        let Cmd::Start(Modulator::Envelope(second)) = second_commands.recv().await.unwrap() else {
            panic!("missing second start")
        };
        assert_eq!(first.duration_ms().await, 20 * 16);
        assert_eq!(second.duration_ms().await, 127 * 16);
        assert_eq!(sent.recv().await.unwrap().msg, midi_note(Channel::Ch1, 100));
        assert!(sent.try_recv().is_err());
        clear_notes(0).await;
    });
}

#[test]
fn core_fans_out_track_notes_and_preserves_constructor_output_channels() {
    let _guard = TEST_NOTES.lock().unwrap();
    block_on(async {
        clear_notes(0).await;
        let (midi, mut sent) = channel();
        let (glide, _) = channel();
        let (envelope, mut started) = channel();
        let (writer, _) = ui_channel();
        let mut core = test_core(midi.clone(), glide, envelope, writer);
        core.voices.truncate(1);
        core.voices
            .push(Voice {
                id: VoiceId(1),
                output_channel: Channel::Ch5,
                midi_sender: midi,
                voicing: heapless::Vec::new(),
                modulations: ::core::array::from_fn(|_| Consumers::Passthrough),
            })
            .unwrap();
        core.tracks[0].main_channel = Channel::Ch2;
        core.tracks[0].aux_channel = Some(Channel::Ch3);
        core.tracks[0].voices.push(VoiceId(1)).unwrap();
        core.process(Input::Midi(midi_note(Channel::Ch2, 100)))
            .await;
        assert!(matches!(
            sent.recv().await.unwrap().msg,
            MidiMsg::ChannelVoice {
                channel: Channel::Ch1,
                msg: ChannelVoiceMsg::NoteOn { .. }
            }
        ));
        assert!(matches!(
            sent.recv().await.unwrap().msg,
            MidiMsg::ChannelVoice {
                channel: Channel::Ch5,
                msg: ChannelVoiceMsg::NoteOn { .. }
            }
        ));
        let Cmd::Start(Modulator::Envelope(mut envelope)) = started.recv().await.unwrap() else {
            panic!("missing routed envelope")
        };
        assert!(matches!(
            envelope.animate(1.0, 1.0, 0.0).await.unwrap()[0],
            MidiMsg::ChannelVoice {
                channel: Channel::Ch1,
                ..
            }
        ));
        // A zero-velocity note arriving on aux input becomes NoteOff in both voices.
        for input in [
            midi_note(Channel::Ch3, 0),
            MidiMsg::RunningChannelVoice {
                channel: Channel::Ch3,
                msg: ChannelVoiceMsg::NoteOn {
                    note: 60,
                    velocity: 0,
                },
            },
        ] {
            core.process(Input::Midi(input)).await;
            for output in [Channel::Ch1, Channel::Ch5] {
                assert_eq!(
                    sent.recv().await.unwrap().msg,
                    MidiMsg::ChannelVoice {
                        channel: output,
                        msg: ChannelVoiceMsg::NoteOff {
                            note: 60,
                            velocity: 0
                        },
                    }
                );
            }
            assert!(started.try_recv().is_err()); // Zero velocity never retriggers an envelope.
        }
        // Unmatched MIDI is preserved, including its zero-velocity NoteOn representation.
        for external in [
            midi_note(Channel::Ch9, 0),
            MidiMsg::SystemRealTime {
                msg: midi_msg::SystemRealTimeMsg::TimingClock,
            },
        ] {
            core.process(Input::Midi(external.clone())).await;
            assert_eq!(sent.recv().await.unwrap().msg, external);
        }
        let control = ChannelVoiceMsg::ControlChange {
            control: ControlChange::CC {
                control: 20,
                value: 55,
            },
        };
        core.process(Input::Midi(MidiMsg::ChannelVoice {
            channel: Channel::Ch2,
            msg: control.clone(),
        }))
        .await;
        for output in [Channel::Ch1, Channel::Ch5] {
            assert_eq!(
                sent.recv().await.unwrap().msg,
                MidiMsg::ChannelVoice {
                    channel: output,
                    msg: control.clone()
                }
            );
        }
        core.process(Input::Midi(MidiMsg::RunningChannelMode {
            channel: Channel::Ch3,
            msg: midi_msg::ChannelModeMsg::AllSoundOff,
        }))
        .await;
        for output in [Channel::Ch1, Channel::Ch5] {
            assert_eq!(
                sent.recv().await.unwrap().msg,
                MidiMsg::ChannelMode {
                    channel: output,
                    msg: midi_msg::ChannelModeMsg::AllSoundOff
                }
            );
        }
        assert!(sent.try_recv().is_err());
        clear_notes(0).await;
    });
}

#[test]
fn encoder_direction_preserves_each_consumers_step_size() {
    block_on(async {
        let (sender, mut commands) = channel();
        let mut glide = Glider::new(VoiceId(0), Channel::Ch1, sender);
        let mut out = OutputEvents::new();
        glide.consume(&turn(false), &mut out).await;
        let _ = commands.recv().await.unwrap();
        assert_eq!(glide.ui_snapshot().duration_ms, 190);
        glide.consume(&turn(true), &mut out).await;
        let _ = commands.recv().await.unwrap();
        assert_eq!(glide.ui_snapshot().duration_ms, 200);

        let (sender, mut commands) = channel();
        let mut envelope = ModTrigger::new(VoiceId(0), Channel::Ch1, 2, sender);
        envelope.consume(&turn(false), &mut out).await;
        let _ = commands.recv().await.unwrap();
        assert_eq!(envelope.ui_snapshot().mode, 2);
        envelope.consume(&turn(true), &mut out).await;
        let _ = commands.recv().await.unwrap();
        assert_eq!(envelope.ui_snapshot().mode, 0);

        let mut octave = consumers::Octave::new();
        octave.consume(&turn(false), &mut out).await;
        assert_eq!(octave.ui_snapshot().shift, -12);
        octave.consume(&turn(true), &mut out).await;
        assert_eq!(octave.ui_snapshot().shift, 0);
        assert!(out.is_empty());
    });
}

struct Producer {
    consumed: bool,
    generated: MidiMsg,
}

impl Consumer for Producer {
    async fn consume(&mut self, _input: &Input, out: &mut OutputEvents) -> bool {
        out.push(self.generated.clone()).unwrap();
        self.consumed
    }
}

impl SliderTarget for Producer {
    fn slider_value(&self, _index: u8, _alternate: bool) -> Option<u8> {
        None
    }
}

#[test]
fn consumption_controls_only_original_and_preserves_generated_midi() {
    block_on(async {
        let original = midi_note(Channel::Ch1, 100);
        let generated = midi_note(Channel::Ch5, 70);
        for consumed in [false, true] {
            let mut producer = SliderCatchUp::new(Producer {
                consumed,
                generated: generated.clone(),
            });
            let mut out = OutputEvents::new();
            voice::dispatch_consumer(&mut producer, Input::Midi(original.clone()), &mut out).await;
            assert_eq!(out[0], generated);
            if consumed {
                assert_eq!(out.len(), 1);
            } else {
                assert_eq!(out.as_slice(), &[generated.clone(), original.clone()]);
            }
            out.clear();
            voice::dispatch_consumer(
                &mut producer,
                Input::EncoderClick { pressed: false },
                &mut out,
            )
            .await;
            assert_eq!(out.as_slice(), &[generated.clone()]); // Hardware never enters output.
        }
    });
}

#[test]
fn octave_replacement_reaches_following_consumers_once() {
    let _guard = TEST_NOTES.lock().unwrap();
    block_on(async {
        clear_notes(0).await;
        let (midi, mut sent) = channel();
        let (glide, _) = channel();
        let (envelope, mut started) = channel();
        let (writer, _) = ui_channel();
        let mut core = test_core(midi, glide, envelope, writer);
        assert!(core.select(TrackId(0), VoiceId(0), 0));
        core.voices[0].voicing[0]
            .consume(&turn(true), &mut OutputEvents::new())
            .await;
        core.process(Input::Midi(midi_note(Channel::Ch1, 100)))
            .await;
        assert_eq!(
            sent.recv().await.unwrap().msg,
            MidiMsg::ChannelVoice {
                channel: Channel::Ch1,
                msg: ChannelVoiceMsg::NoteOn {
                    note: 72,
                    velocity: 100
                },
            }
        );
        assert!(sent.try_recv().is_err());
        assert_eq!(
            notes::read_notes(VoiceId(0), |notes| notes.is_held(72)).await,
            Some(true)
        );
        assert_eq!(
            notes::read_notes(VoiceId(0), |notes| notes.is_held(60)).await,
            Some(false)
        );
        assert!(matches!(
            started.recv().await.unwrap(),
            Cmd::Start(Modulator::Envelope(_))
        ));
        clear_notes(0).await;
    });
}

#[test]
fn voice_sends_unconsumed_midi_controls_on_its_output_channel() {
    block_on(async {
        let (midi, mut sent) = channel();
        let mut consumers = heapless::Vec::new();
        consumers
            .push(Consumers::Octave(consumers::Octave::new()))
            .unwrap();
        let mut voice = Voice {
            id: VoiceId(0),
            output_channel: Channel::Ch5,
            midi_sender: midi,
            voicing: consumers,
            modulations: ::core::array::from_fn(|_| Consumers::Passthrough),
        };
        let control = ChannelVoiceMsg::ControlChange {
            control: ControlChange::CC {
                control: 20,
                value: 55,
            },
        };
        voice
            .process_midi(MidiMsg::RunningChannelVoice {
                channel: Channel::Ch2,
                msg: control.clone(),
            })
            .await;
        assert_eq!(
            sent.recv().await.unwrap().msg,
            MidiMsg::ChannelVoice {
                channel: Channel::Ch5,
                msg: control
            }
        );
        assert!(sent.try_recv().is_err());
        voice
            .process_lane(0, Input::EncoderClick { pressed: false })
            .await;
        assert!(sent.try_recv().is_err());

        // No voicing processors and four inert lanes retain ordinary MIDI output.
        voice.voicing.clear();
        voice.process_midi(midi_note(Channel::Ch2, 100)).await;
        assert_eq!(sent.recv().await.unwrap().msg, midi_note(Channel::Ch5, 100));
        let clock = MidiMsg::SystemRealTime {
            msg: midi_msg::SystemRealTimeMsg::TimingClock,
        };
        voice.process_midi(clock.clone()).await;
        assert_eq!(sent.recv().await.unwrap().msg, clock);
        for action in [
            slider(3, u16::MAX),
            turn(true),
            Input::EncoderClick { pressed: false },
            Input::Button {
                index: 0,
                pressed: true,
            },
            Input::Button {
                index: 0,
                pressed: false,
            },
        ] {
            voice.process_lane(0, action).await;
        }
        assert!(sent.try_recv().is_err());
    });
}

#[test]
fn glide_consumes_legato_notes_and_forwards_final_release() {
    let _guard = TEST_NOTES.lock().unwrap();
    block_on(async {
        clear_notes(0).await;
        let (midi, mut sent) = channel();
        let (glide, mut gliding) = channel();
        let (envelope, mut triggered) = channel();
        let (writer, _) = ui_channel();
        let mut core = test_core(midi, glide, envelope, writer);
        core.process(Input::Midi(midi_note(Channel::Ch1, 100)))
            .await;
        assert_eq!(sent.recv().await.unwrap().msg, midi_note(Channel::Ch1, 100));
        assert!(matches!(
            triggered.recv().await.unwrap(),
            Cmd::Start(Modulator::Envelope(_))
        ));
        core.process(Input::Midi(MidiMsg::ChannelVoice {
            channel: Channel::Ch1,
            msg: ChannelVoiceMsg::NoteOn {
                note: 64,
                velocity: 100,
            },
        }))
        .await;
        assert!(matches!(
            gliding.recv().await.unwrap(),
            Cmd::Start(Modulator::Glide(_))
        ));
        assert!(sent.try_recv().is_err());
        assert!(triggered.try_recv().is_err());
        core.process(Input::Midi(MidiMsg::ChannelVoice {
            channel: Channel::Ch1,
            msg: ChannelVoiceMsg::NoteOff {
                note: 64,
                velocity: 0,
            },
        }))
        .await;
        assert!(matches!(
            gliding.recv().await.unwrap(),
            Cmd::Start(Modulator::Glide(_))
        ));
        assert!(sent.try_recv().is_err());
        let final_release = MidiMsg::ChannelVoice {
            channel: Channel::Ch1,
            msg: ChannelVoiceMsg::NoteOff {
                note: 60,
                velocity: 0,
            },
        };
        core.process(Input::Midi(final_release.clone())).await;
        assert!(matches!(gliding.recv().await.unwrap(), Cmd::Stop));
        assert_eq!(sent.recv().await.unwrap().msg, final_release);
        assert!(sent.try_recv().is_err());
        assert_eq!(
            notes::read_notes(VoiceId(0), |notes| notes.any_held()).await,
            Some(false)
        );
    });
}

fn four_voice_core() -> (
    Core,
    Receiver<'static, MidiOut, 16>,
    rtic_sync::signal::SignalReader<'static, DisplaySnapshot>,
    heapless::Vec<[Receiver<'static, Cmd, 1>; crate::anim::ANIMATORS_PER_VOICE], MAX_VOICES>,
) {
    let (midi, sent) = channel();
    let (writer, reader) = ui_channel();
    let mut receivers = heapless::Vec::new();
    let resources = ::core::array::from_fn(|_| {
        let (glide, glide_rx) = channel();
        let (first, first_rx) = channel();
        let (second, second_rx) = channel();
        receivers.push([glide_rx, first_rx, second_rx]).unwrap();
        crate::anim::VoiceAnimators {
            glide,
            envelopes: [first, second],
        }
    });
    (Core::new(midi, resources, writer), sent, reader, receivers)
}

#[test]
fn four_tracks_route_both_inputs_to_independent_voice_outputs_and_envelopes() {
    let _guard = TEST_NOTES.lock().unwrap();
    block_on(async {
        let (mut core, mut sent, _, mut commands) = four_voice_core();
        assert_eq!(core.tracks.len(), 4);
        assert_eq!(core.voices.len(), 4);
        let outputs = [Channel::Ch1, Channel::Ch2, Channel::Ch3, Channel::Ch4];
        let inputs = [
            [Channel::Ch1, Channel::Ch5],
            [Channel::Ch2, Channel::Ch6],
            [Channel::Ch3, Channel::Ch7],
            [Channel::Ch4, Channel::Ch8],
        ];
        // Hold a note on all four voices concurrently. Each voice must send its
        // first note rather than starting a glide from another voice's note.
        for (index, channels) in inputs.iter().enumerate() {
            clear_notes(index as u8).await;
            core.process(Input::Midi(midi_note(channels[0], 100))).await;
            assert_eq!(
                sent.recv().await.unwrap().msg,
                midi_note(outputs[index], 100)
            );
            assert!(commands[index][0].try_recv().is_err());
            for (envelope_index, cc) in [(1, 2), (2, 3)] {
                let Cmd::Start(Modulator::Envelope(mut envelope)) =
                    commands[index][envelope_index].recv().await.unwrap()
                else {
                    panic!("missing independent envelope start")
                };
                assert_eq!(
                    envelope.animate(1.0, 1.0, 0.0).await.unwrap()[0],
                    MidiMsg::ChannelVoice {
                        channel: outputs[index],
                        msg: ChannelVoiceMsg::ControlChange {
                            control: ControlChange::CC {
                                control: cc,
                                value: 127
                            }
                        },
                    }
                );
            }
        }
        // Aux-channel legato notes start only that voice's glide engine.
        for (index, channels) in inputs.iter().enumerate() {
            core.process(Input::Midi(MidiMsg::ChannelVoice {
                channel: channels[1],
                msg: ChannelVoiceMsg::NoteOn {
                    note: 64,
                    velocity: 100,
                },
            }))
            .await;
            let Cmd::Start(Modulator::Glide(mut glide)) = commands[index][0].recv().await.unwrap()
            else {
                panic!("missing independent glide start")
            };
            assert!(
                glide
                    .animate(1.0, 1.0, 0.0)
                    .await
                    .unwrap()
                    .iter()
                    .all(|message| matches!(
                        message, MidiMsg::ChannelVoice { channel, .. } if *channel == outputs[index]
                    ))
            );
            assert!(sent.try_recv().is_err());
            assert!(commands[index][1].try_recv().is_err());
            assert!(commands[index][2].try_recv().is_err());
        }
        // Passthrough CCs on aux inputs must also use each voice's output channel.
        for (index, channels) in inputs.iter().enumerate() {
            let control = ChannelVoiceMsg::ControlChange {
                control: ControlChange::CC {
                    control: 20,
                    value: 55,
                },
            };
            core.process(Input::Midi(MidiMsg::ChannelVoice {
                channel: channels[1],
                msg: control.clone(),
            }))
            .await;
            assert_eq!(
                sent.recv().await.unwrap().msg,
                MidiMsg::ChannelVoice {
                    channel: outputs[index],
                    msg: control
                }
            );
            clear_notes(index as u8).await;
        }
    });
}

#[test]
fn plain_buttons_select_four_modulation_lanes_and_only_edit_the_selected_lane() {
    block_on(async {
        let (mut core, mut sent, mut reader, mut commands) = four_voice_core();
        let _ = reader.try_read();
        for index in 0..4 {
            let previous = core.selected_lane;
            core.process(Input::Button {
                index,
                pressed: true,
            })
            .await;
            assert_eq!(core.selected_lane, previous);
            assert!(reader.try_read().is_none());
            core.process(Input::Button {
                index,
                pressed: false,
            })
            .await;
            let snapshot = reader
                .try_read()
                .expect("lane selection must refresh the display");
            assert_eq!(
                (snapshot.track, snapshot.voice, snapshot.lane),
                (0, 0, index)
            );
            assert_eq!(core.selected_lane, index as usize);
            if index < 2 {
                assert_eq!(snapshot.model, None);
                core.process(turn(true)).await;
                core.process(slider(3, u16::MAX)).await;
                assert!(reader.try_read().is_none());
            } else {
                assert!(
                    matches!(snapshot.model, Some(UiModel::ModTrigger(model)) if model.output_cc == index)
                );
            }
        }
        for voice in &mut commands {
            for receiver in voice {
                assert!(receiver.try_recv().is_err());
            }
        }
        pickup_slider(&mut core, 3).await;
        core.process(slider(3, u16::MAX)).await;
        assert!(matches!(commands[0][2].recv().await.unwrap(),
            Cmd::Update(Settings::Envelope(settings)) if settings.attack.duration == 127));
        let Some(UiModel::ModTrigger(first)) = core.voices[0].modulations[2].ui_snapshot() else {
            panic!("first envelope missing")
        };
        assert_eq!(first.attack.duration, 20);
        assert!(commands[0][1].try_recv().is_err());
        assert!(sent.try_recv().is_err());
    });
}

#[test]
fn voicing_runs_before_all_four_modulation_lanes_regardless_of_selected_lane() {
    let _guard = TEST_NOTES.lock().unwrap();
    block_on(async {
        clear_notes(0).await;
        let (mut core, mut sent, _, mut commands) = four_voice_core();
        assert_eq!(core.voices[0].voicing.len(), 2);
        assert_eq!(core.voices[0].modulations.len(), MODULATION_LANES);
        core.voices[0].voicing[0]
            .consume(&turn(true), &mut OutputEvents::new())
            .await;
        select_lane_by_button(&mut core, 0).await;
        core.process(Input::Midi(midi_note(Channel::Ch1, 100)))
            .await;
        assert_eq!(
            sent.recv().await.unwrap().msg,
            MidiMsg::ChannelVoice {
                channel: Channel::Ch1,
                msg: ChannelVoiceMsg::NoteOn {
                    note: 72,
                    velocity: 100
                },
            }
        );
        assert!(sent.try_recv().is_err()); // Inert lanes never duplicate or send MIDI themselves.
        assert_eq!(
            notes::read_notes(VoiceId(0), |notes| notes.is_held(72)).await,
            Some(true)
        );
        assert_eq!(
            notes::read_notes(VoiceId(0), |notes| notes.is_held(60)).await,
            Some(false)
        );
        for index in [1, 2] {
            assert!(matches!(
                commands[0][index].recv().await.unwrap(),
                Cmd::Start(Modulator::Envelope(_))
            ));
        }
        assert!(commands[0][0].try_recv().is_err());
        clear_notes(0).await;
    });
}

#[test]
fn encoder_hold_and_button_release_select_tracks_and_edit_only_their_first_envelope() {
    block_on(async {
        let (mut core, _, mut reader, mut commands) = four_voice_core();
        let initial = reader.try_read().unwrap();
        assert_eq!((initial.track, initial.voice), (0, 0));
        for index in [1, 2, 3, 0] {
            core.process(Input::EncoderClick { pressed: true }).await;
            core.process(Input::Button {
                index,
                pressed: true,
            })
            .await;
            assert!(reader.try_read().is_none()); // Never switch on button press.
            core.process(Input::Button {
                index,
                pressed: false,
            })
            .await;
            let selected = reader
                .try_read()
                .expect("selection must redraw identical envelope settings");
            assert_eq!((selected.track, selected.voice), (index, index));
            assert!(
                matches!(selected.model, Some(UiModel::ModTrigger(model)) if model.output_cc == 2 && model.mode == 0)
            );
            assert_eq!(core.selected_lane, 2);
            core.process(Input::EncoderClick { pressed: false }).await;
            assert!(reader.try_read().is_none());
        }
        select_by_button(&mut core, 3).await;
        let selected = reader.try_read().unwrap();
        assert_eq!((selected.track, selected.voice), (3, 3));
        pickup_slider(&mut core, 3).await;
        core.process(slider(3, u16::MAX)).await;
        assert!(
            matches!(commands[3][1].recv().await.unwrap(), Cmd::Update(Settings::Envelope(settings)) if settings.attack.duration == 127)
        );
        assert!(
            matches!(reader.try_read().unwrap().model, Some(UiModel::ModTrigger(model)) if model.attack.duration == 127)
        );
        for (index, voice) in core.voices.iter().enumerate() {
            let Some(UiModel::ModTrigger(first)) = voice.modulations[2].ui_snapshot() else {
                panic!("first envelope missing")
            };
            let Some(UiModel::ModTrigger(second)) = voice.modulations[3].ui_snapshot() else {
                panic!("second envelope missing")
            };
            assert_eq!(first.attack.duration, if index == 3 { 127 } else { 20 });
            assert_eq!(second.attack.duration, 20);
        }
        // Navigation never reaches an envelope as a mode or surface command.
        for receiver in &mut commands {
            for command in receiver {
                assert!(command.try_recv().is_err());
            }
        }
        select_by_button(&mut core, 0).await;
        assert!(
            matches!(reader.try_read().unwrap().model, Some(UiModel::ModTrigger(model)) if model.attack.duration == 20)
        );
        select_by_button(&mut core, 3).await;
        assert!(
            matches!(reader.try_read().unwrap().model, Some(UiModel::ModTrigger(model)) if model.attack.duration == 127)
        );
    });
}

#[test]
fn encoder_hold_edits_curves_and_release_restores_duration_edits() {
    block_on(async {
        let (mut core, mut sent, mut reader, mut commands) = four_voice_core();
        let initial = core.ui_snapshot();
        let _ = reader.try_read();
        core.process(Input::EncoderClick { pressed: true }).await;
        core.process(Input::EncoderClick { pressed: false }).await;
        assert_eq!(core.ui_snapshot(), initial);
        assert!(reader.try_read().is_none());
        assert!(commands[0][1].try_recv().is_err());

        core.process(Input::EncoderClick { pressed: true }).await;
        for index in [3, 2, 0] {
            pickup_slider(&mut core, index).await;
            core.process(slider(index, 0)).await;
            let Cmd::Update(Settings::Envelope(settings)) = commands[0][1].recv().await.unwrap()
            else {
                panic!("missing curve update")
            };
            assert_eq!(
                [
                    settings.attack.duration,
                    settings.decay.duration,
                    settings.release.duration
                ],
                [20; 3]
            );
            let stage = match index {
                3 => settings.attack,
                2 => settings.decay,
                _ => settings.release,
            };
            assert_eq!(stage.curve, 0);
        }
        pickup_slider(&mut core, 1).await;
        core.process(slider(1, 0)).await;
        assert!(matches!(commands[0][1].recv().await.unwrap(),
            Cmd::Update(Settings::Envelope(settings)) if settings.sustain == 0));
        core.process(Input::EncoderClick { pressed: false }).await;
        for index in [3, 2, 0] {
            pickup_slider(&mut core, index).await;
            core.process(slider(index, u16::MAX)).await;
            let Cmd::Update(Settings::Envelope(settings)) = commands[0][1].recv().await.unwrap()
            else {
                panic!("missing duration update")
            };
            assert_eq!(
                [
                    settings.attack.curve,
                    settings.decay.curve,
                    settings.release.curve
                ],
                [0; 3]
            );
            let stage = match index {
                3 => settings.attack,
                2 => settings.decay,
                _ => settings.release,
            };
            assert_eq!(stage.duration, 127);
        }
        assert_eq!(core.selected_track, TrackId(0));
        assert!(sent.try_recv().is_err());
        for voice in &mut commands {
            for receiver in voice {
                assert!(receiver.try_recv().is_err());
            }
        }
    });
}

#[test]
fn curve_modifier_follows_selection_without_sticking_to_a_voice() {
    block_on(async {
        let (mut core, _, _, mut commands) = four_voice_core();
        core.process(Input::EncoderClick { pressed: true }).await;
        core.process(Input::Button {
            index: 2,
            pressed: true,
        })
        .await;
        core.process(Input::Button {
            index: 2,
            pressed: false,
        })
        .await;
        assert_eq!(core.selected_track, TrackId(2));
        pickup_slider(&mut core, 3).await;
        core.process(slider(3, u16::MAX)).await;
        assert!(matches!(commands[2][1].recv().await.unwrap(),
            Cmd::Update(Settings::Envelope(settings)) if settings.attack.curve == 127 && settings.attack.duration == 20));
        core.process(Input::EncoderClick { pressed: false }).await;
        pickup_slider(&mut core, 3).await;
        core.process(slider(3, 0)).await;
        assert!(matches!(commands[2][1].recv().await.unwrap(),
            Cmd::Update(Settings::Envelope(settings)) if settings.attack.curve == 127 && settings.attack.duration == 0));
        let Some(UiModel::ModTrigger(original)) = core.voices[0].modulations[2].ui_snapshot()
        else {
            panic!("original voice model missing")
        };
        let Some(UiModel::ModTrigger(second)) = core.voices[2].modulations[3].ui_snapshot() else {
            panic!("second envelope model missing")
        };
        assert_eq!(original.attack.curve, 64);
        assert_eq!(original.attack.duration, 20);
        assert_eq!(second.attack.curve, 64);
        assert_eq!(second.attack.duration, 20);
    });
}

#[test]
fn catch_up_blocks_updates_and_accepts_crossing_in_both_directions() {
    block_on(async {
        for positions in [[0, 19, 21], [127, 21, 19]] {
            let (sender, mut commands) = channel();
            let mut envelope =
                SliderCatchUp::new(ModTrigger::new(VoiceId(0), Channel::Ch1, 2, sender));
            let initial = envelope.ui_snapshot();
            assert!(envelope.has_updates());
            let mut out = OutputEvents::new();
            for value in &positions[..2] {
                assert!(
                    envelope
                        .consume(&slider(3, control_value(*value)), &mut out)
                        .await
                );
                assert_eq!(envelope.ui_snapshot(), initial);
                assert!(!envelope.has_updates());
                assert!(commands.try_recv().is_err());
                assert!(out.is_empty());
            }
            envelope
                .consume(&slider(3, control_value(positions[2])), &mut out)
                .await;
            assert!(matches!(commands.recv().await.unwrap(),
                Cmd::Update(Settings::Envelope(settings)) if settings.attack.duration == positions[2]));
            assert!(envelope.has_updates());
            // Once picked up, ordinary movement follows in either direction.
            for value in [127, 0] {
                envelope
                    .consume(&slider(3, control_value(value)), &mut out)
                    .await;
                assert!(matches!(commands.recv().await.unwrap(),
                    Cmd::Update(Settings::Envelope(settings)) if settings.attack.duration == value));
            }
        }
    });
}

#[test]
fn catch_up_accepts_exact_values_and_full_scale_endpoints() {
    block_on(async {
        for target in [0, 20, 127] {
            let (sender, mut commands) = channel();
            let mut inner = ModTrigger::new(VoiceId(0), Channel::Ch1, 2, sender);
            let mut out = OutputEvents::new();
            inner
                .consume(&slider(3, control_value(target)), &mut out)
                .await;
            commands.try_recv().ok();
            let mut envelope = SliderCatchUp::new(inner);
            let _ = envelope.has_updates();
            let before = if target == 0 { 127 } else { 0 };
            envelope
                .consume(&slider(3, control_value(before)), &mut out)
                .await;
            assert!(commands.try_recv().is_err());
            envelope
                .consume(&slider(3, control_value(target)), &mut out)
                .await;
            assert!(commands.try_recv().is_err()); // Pickup itself does not change settings.
            assert!(!envelope.has_updates());
            let next = if target == 127 { 126 } else { target + 1 };
            envelope
                .consume(&slider(3, control_value(next)), &mut out)
                .await;
            assert!(matches!(commands.recv().await.unwrap(),
                Cmd::Update(Settings::Envelope(settings)) if settings.attack.duration == next));
        }
    });
}

#[test]
fn envelope_curve_top_is_logarithmic_and_bottom_exponential() {
    async fn value_at(envelope: &mut Envelope, progress: f32) -> u8 {
        let messages = envelope.animate(progress, 1.0, 0.0).await.unwrap();
        let MidiMsg::ChannelVoice {
            msg:
                ChannelVoiceMsg::ControlChange {
                    control: ControlChange::CC { value, .. },
                },
            ..
        } = messages[0]
        else {
            panic!("expected envelope CC")
        };
        value
    }
    block_on(async {
        let mut midpoints = Vec::<[u8; 3], 3>::new();
        for curve in [0, 64, 127] {
            let mut settings = EnvelopeState::INITIAL;
            settings.mode = 2; // ADSR, covering all three ramp stages.
            settings.attack.curve = curve;
            settings.decay.curve = curve;
            settings.release.curve = curve;
            let mut envelope = Envelope::new(VoiceId(0), Channel::Ch1, 2, settings);
            assert_eq!(value_at(&mut envelope, 0.0).await, 0);
            assert_eq!(value_at(&mut envelope, 1.0).await, 127);
            let attack = value_at(&mut envelope, 0.5).await;
            assert!(envelope.next_stage().await);
            assert_eq!(value_at(&mut envelope, 0.0).await, 127);
            assert_eq!(value_at(&mut envelope, 1.0).await, 80);
            let decay = value_at(&mut envelope, 0.5).await;
            assert!(envelope.next_stage().await); // Sustain hold.
            assert!(envelope.next_stage().await); // Release ramp.
            assert_eq!(value_at(&mut envelope, 0.0).await, 80);
            let release = value_at(&mut envelope, 0.5).await;
            assert_eq!(value_at(&mut envelope, 1.0).await, 0);
            midpoints.push([attack, decay, release]).unwrap();
        }
        let [bottom, center, top] = midpoints.as_slice() else {
            unreachable!()
        };
        assert_eq!(*center, [63, 103, 40]); // Curve 64 stays linear.
        assert!(bottom[0] < center[0] && top[0] > center[0]);
        assert!(bottom[1] < center[1] && top[1] > center[1]);
        assert!(bottom[2] < center[2] && top[2] > center[2]);
        assert!(bottom[1] > 80 && top[1] < 127);
        assert!(bottom[2] > 0 && top[2] < 80);
    });
}

#[test]
fn curves_bypass_pickup_but_duration_and_sustain_still_wait() {
    block_on(async {
        let (mut core, _, _, mut commands) = four_voice_core();
        core.process(Input::EncoderClick { pressed: true }).await;
        for index in [3, 2, 0] {
            core.process(slider(index, u16::MAX)).await;
            let Cmd::Update(Settings::Envelope(settings)) = commands[0][1].recv().await.unwrap()
            else {
                panic!("curve must update immediately without pickup")
            };
            let stage = match index {
                3 => settings.attack,
                2 => settings.decay,
                _ => settings.release,
            };
            assert_eq!(stage.curve, 127);
            assert_eq!(stage.duration, 20);
        }
        core.process(slider(1, 0)).await;
        assert!(commands[0][1].try_recv().is_err()); // Sustain still waits for its value.
        core.process(slider(1, control_value(81))).await;
        assert!(matches!(commands[0][1].recv().await.unwrap(),
            Cmd::Update(Settings::Envelope(settings)) if settings.sustain == 81));
        core.process(Input::EncoderClick { pressed: false }).await;
        core.process(slider(3, control_value(126))).await;
        assert!(commands[0][1].try_recv().is_err()); // Curve movement must not unlock duration.
        core.process(slider(3, control_value(19))).await;
        assert!(matches!(commands[0][1].recv().await.unwrap(),
            Cmd::Update(Settings::Envelope(settings)) if settings.attack.duration == 19 && settings.attack.curve == 127));
    });
}

#[test]
fn releasing_curve_modifier_waits_for_duration_pickup_instead_of_jumping() {
    block_on(async {
        let (mut core, _, mut reader, mut commands) = four_voice_core();
        pickup_slider(&mut core, 3).await;
        core.process(slider(3, control_value(25))).await;
        let _ = commands[0][1].recv().await.unwrap();
        core.process(Input::EncoderClick { pressed: true }).await;
        pickup_slider(&mut core, 3).await;
        core.process(slider(3, u16::MAX)).await;
        let _ = commands[0][1].recv().await.unwrap();
        core.process(Input::EncoderClick { pressed: false }).await;
        let _ = reader.try_read();
        for value in [126, 26] {
            core.process(slider(3, control_value(value))).await;
            assert!(commands[0][1].try_recv().is_err());
            assert!(reader.try_read().is_none());
            assert!(matches!(core.ui_snapshot().model,
                Some(UiModel::ModTrigger(model)) if model.attack.duration == 25 && model.attack.curve == 127));
        }
        core.process(slider(3, control_value(24))).await;
        assert!(matches!(commands[0][1].recv().await.unwrap(),
            Cmd::Update(Settings::Envelope(settings)) if settings.attack.duration == 24 && settings.attack.curve == 127));
    });
}

#[test]
fn track_and_consumer_changes_rearm_pickup_using_current_physical_position() {
    block_on(async {
        let (mut core, _, _, mut commands) = four_voice_core();
        pickup_slider(&mut core, 3).await;
        core.process(slider(3, control_value(100))).await;
        let _ = commands[0][1].recv().await.unwrap();
        assert!(core.select(TrackId(1), VoiceId(1), 2));
        core.process(slider(3, control_value(99))).await;
        assert!(commands[1][1].try_recv().is_err());
        core.process(slider(3, control_value(19))).await;
        let _ = commands[1][1].recv().await.unwrap();

        assert!(core.select(TrackId(0), VoiceId(0), 2));
        core.process(slider(3, control_value(20))).await;
        assert!(commands[0][1].try_recv().is_err()); // Old physical position was 100; current is 19.
        core.process(slider(3, control_value(101))).await;
        assert!(matches!(commands[0][1].recv().await.unwrap(),
            Cmd::Update(Settings::Envelope(settings)) if settings.attack.duration == 101));
        assert!(core.select(TrackId(0), VoiceId(0), 3));
        core.process(slider(3, control_value(100))).await;
        assert!(commands[0][2].try_recv().is_err());
        core.process(slider(3, control_value(19))).await;
        assert!(matches!(commands[0][2].recv().await.unwrap(),
            Cmd::Update(Settings::Envelope(settings)) if settings.attack.duration == 19));
        assert!(core.select(TrackId(0), VoiceId(0), 3)); // Selecting the same page must not re-arm.
        core.process(slider(3, control_value(30))).await;
        assert!(matches!(commands[0][2].recv().await.unwrap(),
            Cmd::Update(Settings::Envelope(settings)) if settings.attack.duration == 30));
    });
}

#[test]
fn encoder_rotation_edits_selected_envelope_instead_of_changing_tracks() {
    block_on(async {
        let (mut core, mut sent, mut reader, mut commands) = four_voice_core();
        let _ = reader.try_read();
        core.process(turn(true)).await;
        assert_eq!(core.selected_track, TrackId(0));
        assert!(matches!(
            reader.try_read().unwrap().model,
            Some(UiModel::ModTrigger(model)) if model.mode == 1
        ));
        assert!(matches!(commands[0][1].recv().await.unwrap(),
            Cmd::Update(Settings::Envelope(settings)) if settings.mode == 1));
        for voice in &mut commands {
            for receiver in voice {
                assert!(receiver.try_recv().is_err());
            }
        }

        // Core-level navigation must not intercept musical MIDI while held.
        core.process(Input::EncoderClick { pressed: true }).await;
        let clock = MidiMsg::SystemRealTime {
            msg: midi_msg::SystemRealTimeMsg::TimingClock,
        };
        core.process(Input::Midi(clock.clone())).await;
        assert_eq!(sent.recv().await.unwrap().msg, clock);
        assert!(sent.try_recv().is_err());
    });
}

#[test]
fn ui_providers_acknowledge_changes_once_without_consuming_latest_model() {
    block_on(async {
        let mut octave = consumers::Octave::new();
        let mut out = OutputEvents::new();
        let initial = octave.ui_snapshot();
        assert_eq!(initial, octave.ui_snapshot());
        assert!(octave.has_updates()); // Reading the initial model did not acknowledge it.
        assert!(!octave.has_updates());
        octave.consume(&turn(true), &mut out).await;
        octave.consume(&turn(true), &mut out).await;
        assert_eq!(octave.ui_snapshot().shift, 24);
        assert!(octave.has_updates()); // Both changes coalesced into one notification.
        assert!(!octave.has_updates());
        assert_eq!(octave.ui_snapshot().shift, 24); // Latest model survives acknowledgement.
        octave.consume(&note_on(), &mut out).await;
        assert!(!octave.has_updates()); // MIDI processing changed no displayed settings.
        octave.consume(&slider(3, u16::MAX), &mut out).await;
        assert!(octave.has_updates());
        octave.consume(&slider(3, u16::MAX), &mut out).await;
        assert!(!octave.has_updates()); // Repeating the same setting is not a change.

        let (sender, _) = channel();
        let mut glide = Glider::new(VoiceId(0), Channel::Ch1, sender);
        assert!(glide.has_updates());
        assert!(!glide.has_updates());
        glide.consume(&turn(true), &mut out).await;
        assert_eq!(glide.ui_snapshot().duration_ms, 210);
        assert!(glide.has_updates());
        assert!(!glide.has_updates());

        let (sender, _) = channel();
        let mut envelope = ModTrigger::new(VoiceId(0), Channel::Ch1, 2, sender);
        assert!(envelope.has_updates());
        assert!(!envelope.has_updates());
        envelope.consume(&slider(3, u16::MAX), &mut out).await;
        assert_eq!(envelope.ui_snapshot().attack.duration, 127);
        assert!(envelope.has_updates());
        assert!(!envelope.has_updates());
        envelope.consume(&slider(3, u16::MAX), &mut out).await;
        assert!(!envelope.has_updates());
    });
}

#[test]
fn page_switch_reads_latest_pending_model_and_acknowledges_without_duplicate() {
    block_on(async {
        let (mut core, _, mut reader, mut commands) = four_voice_core();
        let _ = reader.try_read();
        assert!(!core.voices[0].modulations[2].has_updates()); // Initial publication acknowledged it.
        assert!(core.voices[1].modulations[2].has_updates());
        let mut out = OutputEvents::new();
        core.voices[1].modulations[2]
            .consume(&slider(3, control_value(20)), &mut out)
            .await;
        for value in [0, u16::MAX] {
            core.voices[1].modulations[2]
                .consume(&slider(3, value), &mut out)
                .await;
            let _ = commands[1][1].recv().await.unwrap();
        }
        core.publish_ui(); // Publishing the active page must leave other providers pending.
        assert!(reader.try_read().is_none());
        core.process(Input::Select {
            track: TrackId(1),
            voice: VoiceId(1),
            lane: 2,
        })
        .await;
        let latest = reader.try_read().unwrap();
        assert_eq!((latest.track, latest.voice), (1, 1));
        assert!(
            matches!(latest.model, Some(UiModel::ModTrigger(model)) if model.attack.duration == 127)
        );
        assert!(!core.voices[1].modulations[2].has_updates());
        core.publish_ui();
        assert!(reader.try_read().is_none());

        // Neither provider has changed. Returning to its page still publishes current data.
        assert!(core.select(TrackId(0), VoiceId(0), 2));
        assert!(
            matches!(reader.try_read().unwrap().model, Some(UiModel::ModTrigger(model)) if model.attack.duration == 20)
        );
        assert!(core.select(TrackId(1), VoiceId(1), 2));
        assert!(
            matches!(reader.try_read().unwrap().model, Some(UiModel::ModTrigger(model)) if model.attack.duration == 127)
        );
        core.publish_ui();
        assert!(reader.try_read().is_none());
    });
}

#[test]
fn usb_status_updates_global_display_without_changing_the_selected_page() {
    block_on(async {
        let (mut core, mut sent, mut reader, mut commands) = four_voice_core();
        let initial = reader.try_read().unwrap();
        assert!(!initial.usb_connected);
        core.process(Input::UsbConnection { connected: false })
            .await;
        assert!(reader.try_read().is_none());
        core.process(Input::UsbConnection { connected: true }).await;
        let connected = reader
            .try_read()
            .expect("USB status must publish without a parameter edit");
        assert!(connected.usb_connected);
        assert_eq!(connected.model, initial.model);
        assert_eq!(
            (connected.track, connected.voice),
            (initial.track, initial.voice)
        );
        assert_eq!(core.ui_snapshot(), connected); // Full model remains available after publication.
        assert!(!core.has_updates()); // Publication acknowledged the change.
        core.process(Input::UsbConnection { connected: true }).await;
        assert!(reader.try_read().is_none());

        select_by_button(&mut core, 1).await;
        let switched = reader.try_read().unwrap();
        assert_eq!((switched.track, switched.voice), (1, 1));
        assert!(switched.usb_connected); // Status survives page switches.
        core.process(Input::UsbConnection { connected: false })
            .await;
        let disconnected = reader.try_read().unwrap();
        assert!(!disconnected.usb_connected);
        assert_eq!(disconnected.model, switched.model);
        assert_eq!((disconnected.track, disconnected.voice), (1, 1));
        core.process(Input::UsbConnection { connected: false })
            .await;
        assert!(reader.try_read().is_none());
        core.process(Input::UsbConnection { connected: true }).await;
        assert!(reader.try_read().unwrap().usb_connected);
        assert!(sent.try_recv().is_err());
        for voice in &mut commands {
            for receiver in voice {
                assert!(receiver.try_recv().is_err());
            }
        }
    });
}
