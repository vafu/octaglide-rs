use super::{Consumer, EnvelopeMode, Input, Selection, VoiceId, VoiceOutput};
use crate::{
    calc,
    core::{Cmd, Settings, consumers::ModTrigger},
    output::OutputMessage,
};
use futures::executor::block_on;
use rtic_sync::channel::{Channel as Queue, Receiver, Sender};
use std::{boxed::Box, sync::Mutex};

static TEST_NOTES: Mutex<()> = Mutex::new(());

fn channel<T, const N: usize>() -> (Sender<'static, T, N>, Receiver<'static, T, N>) {
    Box::leak(Box::new(Queue::<T, N>::new())).split()
}

fn animator() -> crate::app::Animator {
    channel().0
}

fn voice_outputs() -> (Sender<'static, OutputMessage, 32>, Receiver<'static, OutputMessage, 32>) {
    channel()
}

fn note_on(voice: VoiceId, note: u8) -> Input {
    Input::NoteOn {
        voice,
        note,
        velocity: 100,
    }
}

#[test]
fn semantic_control_slots_map_to_envelope_controls() {
    let _guard = TEST_NOTES.lock().unwrap();
    block_on(async {
        let (sender, mut commands) = channel();
        let mut envelope = ModTrigger::new(VoiceId(0), 0, sender);
        let mut out = super::OutputEvents::new();
        envelope
            .consume(
                &Input::ControlUpdate {
                    voice: VoiceId(0),
                    lane: 0,
                    control: 0,
                    value: 1.0,
                },
                &mut out,
            )
            .await;
        assert_eq!(envelope.control_slot_value(0), Some(1.0));
        assert!(matches!(
            commands.recv().await,
            Ok(Cmd::Update(Settings::Envelope(_)))
        ));
    });
}

#[test]
fn mode_updates_cycle_only_the_targeted_consumer() {
    let _guard = TEST_NOTES.lock().unwrap();
    block_on(async {
        let (sender, _) = channel();
        let mut envelope = ModTrigger::new(VoiceId(0), 0, sender);
        assert_eq!(envelope.mode(), EnvelopeMode::Ad);
        envelope.update_mode(1);
        assert_eq!(envelope.mode(), EnvelopeMode::Ar);
        envelope.update_mode(1);
        assert_eq!(envelope.mode(), EnvelopeMode::Adsr);
        envelope.update_mode(1);
        assert_eq!(envelope.mode(), EnvelopeMode::Ad);
        envelope.update_mode(-1);
        assert_eq!(envelope.mode(), EnvelopeMode::Adsr);
    });
}

#[test]
fn octave_transforms_note_events_without_midi_channels() {
    block_on(async {
        let mut octave = crate::core::consumers::Octave::new(VoiceId(0));
        octave.set_control_slot(0, 1.0);
        let mut out = super::OutputEvents::new();
        octave.consume(&note_on(VoiceId(0), 60), &mut out).await;
        assert_eq!(
            out[0],
            VoiceOutput::NoteOn {
                voice: VoiceId(0),
                note: 96,
                velocity: 100
            }
        );
    });
}

#[test]
fn invalid_selection_is_rejected() {
    let _guard = TEST_NOTES.lock().unwrap();
    block_on(async {
        let (output, _receiver) = voice_outputs();
        let resources = std::array::from_fn(|_| crate::core::VoiceAnimators {
            glide: animator(),
            envelopes: [animator(), animator()],
        });
        let mut core = super::Core::new(output, resources);
        assert!(!core.select(Selection {
            voice: VoiceId(9),
            lane: 0,
        }));
        assert!(core.select(Selection {
            voice: VoiceId(0),
            lane: 2,
        }));
    });
}

#[test]
fn shared_calc_matches_playback_curve_samples() {
    let mut curve = 0;
    while curve < 128 {
        for progress in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let playback = calc::envelope_level(progress, curve, 0.0, 1.0);
            let preview = calc::envelope_level(progress, curve, 0.0, 1.0);
            assert!((playback - preview).abs() < f32::EPSILON);
        }
        curve += 31;
    }
}
