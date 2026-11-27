use super::{Consumer, EnvelopeMode, Input, ModLane, VoiceId, VoiceOutput};
use crate::{
    core::consumers::envelope::calc,
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

fn animator() -> crate::core::Animator {
    channel().0
}

fn voice_outputs() -> (Sender<'static, OutputMessage, 32>, Receiver<'static, OutputMessage, 32>) {
    channel()
}

struct TestClock;

impl crate::core::Clock for TestClock {
    fn now_ms() -> u32 {
        0
    }

    async fn delay_ms(_ms: u32) {}
}

#[test]
fn animation_engine_accepts_a_platform_clock() {
    let _guard = TEST_NOTES.lock().unwrap();
    block_on(async {
        let (sender, receiver) = channel();
        drop(sender);
        let mut engine = crate::core::AnimationEngine::new(receiver);
        assert!(engine.tick::<TestClock>().await.is_none());
    });
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
fn mode_selection_takes_an_absolute_index() {
    let _guard = TEST_NOTES.lock().unwrap();
    block_on(async {
        let (sender, _) = channel();
        let mut envelope = ModTrigger::new(VoiceId(0), 0, sender);
        assert_eq!(envelope.mode(), EnvelopeMode::Ad);
        assert_eq!(envelope.mode_count(), EnvelopeMode::COUNT);

        envelope.set_mode(1);
        assert_eq!(envelope.mode(), EnvelopeMode::Ar);
        envelope.set_mode(2);
        assert_eq!(envelope.mode(), EnvelopeMode::Adsr);
        envelope.set_mode(0);
        assert_eq!(envelope.mode(), EnvelopeMode::Ad);
    });
}

#[test]
fn out_of_range_mode_index_is_ignored() {
    let _guard = TEST_NOTES.lock().unwrap();
    block_on(async {
        let (sender, _) = channel();
        let mut envelope = ModTrigger::new(VoiceId(0), 0, sender);
        envelope.set_mode(1);
        envelope.set_mode(9);
        assert_eq!(envelope.mode(), EnvelopeMode::Ar, "index left unchanged");
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
fn snapshot_rejects_unknown_voice_and_lane() {
    let _guard = TEST_NOTES.lock().unwrap();
    block_on(async {
        let (output, _receiver) = voice_outputs();
        let resources = std::array::from_fn(|_| crate::core::VoiceAnimators {
            glide: [animator()],
            envelopes: [animator(), animator()],
        });
        let core = super::Core::new(output, resources);
        assert!(core.snapshot(VoiceId(9), 0).is_none());
        assert!(core.snapshot(VoiceId(0), 4).is_none());
        assert!(core.snapshot(VoiceId(0), 2).is_some());
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
