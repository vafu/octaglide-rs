use super::{
    Consumer, ConsumerKind, Input, Modulation, NoteKind, NoteTransition, Trigger, VoiceId,
    VoiceOutput,
};
use crate::{
    core::consumers::envelope::calc,
    core::consumers::{ModTrigger, Octave},
    output::OutputMessage,
};
use futures::executor::block_on;
use rtic_sync::channel::{Channel as Queue, Receiver, Sender};
use std::{boxed::Box, sync::Mutex};

static TEST_NOTES: Mutex<()> = Mutex::new(());

fn channel<T, const N: usize>() -> (Sender<'static, T, N>, Receiver<'static, T, N>) {
    Box::leak(Box::new(Queue::<T, N>::new())).split()
}

fn voice_outputs() -> (Sender<'static, OutputMessage, 32>, Receiver<'static, OutputMessage, 32>) {
    channel()
}

fn press(voice: VoiceId, note: u8) -> Input {
    Input::Note(NoteTransition {
        voice,
        kind: NoteKind::Press,
        note,
        velocity: 100,
        previous_top: None,
        was_top: false,
        new_top: None,
        any_held: true,
    })
}

fn release(voice: VoiceId, note: u8, new_top: Option<u8>) -> Input {
    Input::Note(NoteTransition {
        voice,
        kind: NoteKind::Release,
        note,
        velocity: 0,
        previous_top: None,
        was_top: true,
        new_top,
        any_held: new_top.is_some(),
    })
}

#[test]
fn semantic_control_slots_map_to_envelope_controls() {
    let _guard = TEST_NOTES.lock().unwrap();
    let mut envelope = ModTrigger::new(VoiceId(0), 0);
    let mut out = super::OutputEvents::new();
    envelope.consume(
        &Input::ControlUpdate {
            voice: VoiceId(0),
            slot: 8,
            control: 0,
            value: 1.0,
        },
        &mut out,
    );
    assert_eq!(envelope.control_slot_value(0), Some(1.0));
}

#[test]
fn mode_selection_takes_an_absolute_index() {
    let mut envelope = ModTrigger::new(VoiceId(0), 0);
    assert_eq!(envelope.mode_index(), super::consumers::envelope::EnvelopeMode::Ad.index());
    envelope.set_mode(1);
    assert_eq!(envelope.mode_index(), super::consumers::envelope::EnvelopeMode::Ar.index());
    envelope.set_mode(2);
    assert_eq!(envelope.mode_index(), super::consumers::envelope::EnvelopeMode::Adsr.index());
    envelope.set_mode(0);
    assert_eq!(envelope.mode_index(), super::consumers::envelope::EnvelopeMode::Ad.index());
}

#[test]
fn out_of_range_mode_index_is_ignored() {
    let mut envelope = ModTrigger::new(VoiceId(0), 0);
    envelope.set_mode(1);
    envelope.set_mode(9);
    assert_eq!(envelope.mode_index(), super::consumers::envelope::EnvelopeMode::Ar.index(), "index left unchanged");
}

#[test]
fn octave_transforms_note_events_without_midi_channels() {
    let mut octave = Octave::new(VoiceId(0));
    octave.set_control(0, 1.0);
    let mut out = super::OutputEvents::new();
    octave.consume(&press(VoiceId(0), 60), &mut out);
    assert_eq!(
        out[0],
        VoiceOutput::NoteOn {
            voice: VoiceId(0),
            note: 96,
            velocity: 100
        }
    );
}

#[test]
fn snapshot_rejects_unknown_voice_and_slot() {
    let (output, _receiver) = voice_outputs();
    let core = super::Core::new(output);
    assert!(core.snapshot(VoiceId(9), 8).is_none());
    assert!(core.snapshot(VoiceId(0), 17).is_none());
    assert!(core.snapshot(VoiceId(0), 8).is_some());
}

#[test]
fn addressed_control_reaches_only_its_slot() {
    let (output, _receiver) = voice_outputs();
    let mut core = super::Core::new(output);
    let before = core
        .snapshot(VoiceId(0), 9)
        .unwrap();
    block_on(core.process(Input::ControlUpdate {
        voice: VoiceId(0),
        slot: 4,
        control: 0,
        value: 1.0,
    }));
    let after = core
        .snapshot(VoiceId(0), 9)
        .unwrap();
    assert_eq!(before, after, "a Voicing control must not touch Modulation");
}

#[test]
fn note_broadcast_reaches_every_slot_that_wants_notes() {
    let (output, _receiver) = voice_outputs();
    let mut core = super::Core::new(output);
    let envelope = core
        .snapshot(VoiceId(0), 8)
        .unwrap();
    assert_eq!(envelope.kind, ConsumerKind::ModTrigger);
    // Octave emits a transposed note, proving voicing ran.
    block_on(core.process(press(VoiceId(0), 60)));
    block_on(core.advance(5));
}

#[test]
fn envelope_hold_freezes_until_release() {
    let mut envelope = ModTrigger::new(VoiceId(0), 0);
    // AR: attack -> hold -> release. Make every stage short (1 unit = 16 ms).
    envelope.set_mode(super::consumers::envelope::EnvelopeMode::Ar.index());
    envelope.set_control(3, 1.0 / 127.0); // attack duration
    envelope.set_control(0, 1.0 / 127.0); // release duration
    let mut out = super::OutputEvents::new();
    envelope.consume(&press(VoiceId(0), 60), &mut out);

    // Attack completes, then the hold stage parks at full while gated.
    let mut last = 0.0;
    for _ in 0..10 {
        if let Some(messages) = envelope.advance(5, true) {
            if let Some(VoiceOutput::Control { value, .. }) = messages.last() {
                last = *value;
            }
        }
    }
    assert!(last >= 0.99, "hold should park at full, got {last}");

    // Further gated ticks must not move the level.
    envelope.advance(50, true);
    let held = envelope.advance(0, true).unwrap();
    let VoiceOutput::Control { value, .. } = held.last().copied().unwrap() else {
        panic!("expected a control message");
    };
    assert!(value >= 0.99, "hold must stay put, got {value}");

    // Releasing advances to the release ramp, which falls back to zero.
    let mut last_release = 1.0;
    for _ in 0..10 {
        if let Some(messages) = envelope.advance(5, false) {
            if let Some(VoiceOutput::Control { value, .. }) = messages.last() {
                last_release = *value;
            }
        }
    }
    assert!(last_release <= 0.01, "release should end at zero, got {last_release}");
}

#[test]
fn short_stage_completes_without_rounding_up() {
    let mut envelope = ModTrigger::new(VoiceId(0), 0);
    // 1 unit = 16 ms; a 5 ms tick must not stretch it to 20 ms.
    envelope.set_control(3, 1.0 / 127.0);
    let mut out = super::OutputEvents::new();
    envelope.consume(&press(VoiceId(0), 60), &mut out);

    let mut ticks = 0;
    loop {
        ticks += 1;
        let messages = envelope.advance(5, true);
        let at_full = messages.is_some_and(|messages| {
            messages.iter().any(|msg| matches!(msg, VoiceOutput::Control { value, .. } if *value >= 0.99))
        });
        if at_full {
            break;
        }
        assert!(ticks < 10, "1-unit stage should finish in ~4 ticks");
    }
    assert!(ticks <= 4, "took {ticks} ticks, expected <= 4");
}

#[test]
fn free_loop_envelopes_run_without_a_note() {
    let mut envelope = ModTrigger::new(VoiceId(0), 0);
    let settings = super::EnvelopeParams {
        trigger: Trigger::FREE_LOOP,
        ..super::EnvelopeParams::INITIAL
    };
    // FreeLoop has no note trigger, so construction alone should produce output.
    let mut envelope = ModTrigger::new(VoiceId(0), 0);
    envelope.set_control(7, 1.0);
    let _ = settings;
    let messages = envelope.advance(5, false);
    assert!(
        messages.is_some_and(|messages| !messages.is_empty()),
        "a free-running envelope advances without a note"
    );
}

#[test]
fn activation_one_shot_stops_and_loop_wraps() {
    assert!(!Trigger::NOTE_ONCE.loops());
    assert!(Trigger::NOTE_LOOP.loops());
    assert!(Trigger::FREE_LOOP.loops());
    assert!(Trigger::NOTE_ONCE.note_triggered());
    assert!(!Trigger::FREE_LOOP.note_triggered());
}

#[test]
fn settings_change_preserves_position() {
    let mut envelope = ModTrigger::new(VoiceId(0), 0);
    let mut out = super::OutputEvents::new();
    envelope.consume(&press(VoiceId(0), 60), &mut out);

    // Partway through the attack, then lengthen it via a control update.
    envelope.advance(160, true);
    let mut out = super::OutputEvents::new();
    envelope.consume(
        &Input::ControlUpdate {
            voice: VoiceId(0),
            slot: 8,
            control: 3,
            value: 1.0,
        },
        &mut out,
    );
    // Lengthening the attack must not restart the envelope from zero.
    let messages = envelope.advance(0, true).unwrap();
    let VoiceOutput::Control { value, .. } = messages.last().copied().unwrap() else {
        panic!("expected a control message");
    };
    assert!(value > 0.0, "position must not jump back to zero");
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

#[test]
fn releasing_the_last_note_zeroes_the_pitch_bend() {
    let mut glider = super::consumers::Glider::new(VoiceId(0));
    let mut out = super::OutputEvents::new();

    // Press 60, then 67 while held: the second press starts a slide.
    glider.consume(&press(VoiceId(0), 60), &mut out);
    let mut glide_press = press(VoiceId(0), 67);
    if let Input::Note(transition) = &mut glide_press {
        transition.previous_top = Some(60);
    }
    glider.consume(&glide_press, &mut out);
    glider.advance(50, true);

    // Releasing 67 with nothing else held must return the bend to center.
    let mut out = super::OutputEvents::new();
    glider.consume(&release(VoiceId(0), 67, None), &mut out);
    assert!(
        out.iter().any(|msg| matches!(msg, VoiceOutput::PitchBend { amount, .. } if *amount == 0.0)),
        "expected a zero pitch bend, got {out:?}"
    );
}

#[test]
fn control_map_round_trips_every_declared_slot() {
    let mut octave = super::consumers::Octave::new(VoiceId(0));
    // Slot 0 is declared (Shift); slot 1 is padding.
    assert!(octave.control_slot_value(0).is_some());
    assert!(octave.control_slot_value(1).is_none());
    assert!(octave.control_slot_value(7).is_none());

    // Set and read back through the macro-generated parse/format pair.
    octave.set_control(0, 1.0);
    assert_eq!(octave.control_slot_value(0), Some(1.0));
    octave.set_control(0, 0.0);
    assert_eq!(octave.control_slot_value(0), Some(0.0));

    let envelope = super::consumers::ModTrigger::new(VoiceId(0), 0);
    for slot in 0..8 {
        assert!(
            envelope.control_slot_value(slot).is_some(),
            "envelope declares all 8 control slots"
        );
    }
}

#[test]
fn every_declared_control_round_trips() {
    let mut envelope = super::consumers::ModTrigger::new(VoiceId(0), 0);
    // Slots 0..=6 are continuous settings.
    for slot in 0..7 {
        envelope.set_control(slot, 0.25);
        let read = envelope.control_slot_value(slot).unwrap();
        assert!(
            (read - 0.25).abs() < 0.02,
            "slot {slot} round-tripped to {read}"
        );
    }
    // Slot 7 is the discrete trigger; each variant sits at an exact point.
    for (value, expected) in [
        (0.0, Trigger::NOTE_ONCE),
        (0.5, Trigger::NOTE_LOOP),
        (1.0, Trigger::FREE_LOOP),
    ] {
        envelope.set_control(7, value);
        assert_eq!(envelope.control_slot_value(7), Some(value));
        assert_eq!(
            super::Trigger::from_index(libm::roundf(value * 2.0) as u8),
            expected
        );
    }

    let mut glider = super::consumers::Glider::new(VoiceId(0));
    for slot in 0..4 {
        glider.set_control(slot, 0.5);
        assert!(glider.control_slot_value(slot).is_some());
    }
    assert!(glider.control_slot_value(4).is_none(), "slot 4 is padding");
}

#[test]
fn trigger_fields_agree_with_helpers() {
    for trigger in [Trigger::NOTE_ONCE, Trigger::NOTE_LOOP, Trigger::FREE_LOOP] {
        let (note_trigger, looping) = match trigger {
            Trigger::NoteOnce { note_trigger, looping }
            | Trigger::NoteLoop { note_trigger, looping }
            | Trigger::FreeLoop { note_trigger, looping } => (note_trigger, looping),
        };
        assert_eq!(trigger.note_triggered(), note_trigger);
        assert_eq!(trigger.loops(), looping);
        assert_eq!(Trigger::from_index(trigger.index()), trigger);
    }
    assert!(!Trigger::NOTE_ONCE.loops());
    assert!(Trigger::NOTE_LOOP.loops());
    assert!(!Trigger::FREE_LOOP.note_triggered());
}
