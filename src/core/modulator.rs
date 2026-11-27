//! Modulation engines and the single registry that declares them.
//!
//! The `modulation!` invocation at the bottom of this file is the one place an
//! animator-backed modulation is declared. It derives the `Modulator` and
//! `Settings` enums, the per-voice animator slots, and each consumer's control
//! table from that single entry.

use core::fmt::Debug;

use enum_dispatch::enum_dispatch;
use heapless::Vec;
use rtic_sync::channel::Channel;

use super::animator::{AnimationEngine, Animator, Cmd};
use super::consumers::envelope::Envelope;
use super::consumers::glide::Glide;
use super::consumers::{Glider, ModTrigger, CONTROL_SLOTS};
use super::VoiceOutput;

pub type Messages = Option<Vec<VoiceOutput, 3>>;

/// Places each `(slot, control)` pair into a fixed-size slot table.
///
/// Unlisted slots stay `None`, so callers never spell out the padding.
const fn build_control_map<C: Copy, const N: usize>(entries: &[(u8, C)]) -> [Option<C>; N] {
    let mut map = [None; N];
    let mut index = 0;
    while index < entries.len() {
        let (slot, control) = entries[index];
        assert!((slot as usize) < N);
        map[slot as usize] = Some(control);
        index += 1;
    }
    map
}

macro_rules! count {
    () => { 0usize };
    ($head:literal $($tail:literal)*) => { 1usize + count!($($tail)*) };
}

fn take_slots<T, const N: usize>(source: &mut impl Iterator<Item = T>) -> [T; N] {
    core::array::from_fn(|_| source.next().expect("animator slot count"))
}

macro_rules! modulation {
    (
        $(
            $variant:ident {
                animation: $animation:ident,
                consumer: $consumer:ident,
                settings: $settings:ty,
                animator: $field:ident,
                slots: [ $($slot:literal),* $(,)? ],
                controls: [ $( ($control_slot:literal, $control:ident) ),* $(,)? ],
            }
        )*
    ) => {
        #[enum_dispatch(Modulation)]
        #[derive(Debug)]
        pub enum Modulator {
            $( $variant($animation), )*
        }

        #[derive(Clone, Copy, Debug)]
        pub enum Settings {
            $( $variant($settings), )*
        }

        /// One animator command channel per voice.
        pub const ANIMATORS_PER_VOICE: usize = 0usize $( + count!($($slot)*) )*;

        /// Senders handed to `Core`, grouped by modulation.
        #[derive(Debug)]
        pub struct VoiceAnimators {
            $( pub $field: [Animator; [$( $slot as usize ),*].len()], )*
        }

        /// Splits the per-voice animator channels into senders and engines.
        pub fn split_animator_slots(
            channels: &'static mut [Channel<Cmd, 1>; ANIMATORS_PER_VOICE],
        ) -> (VoiceAnimators, [AnimationEngine; ANIMATORS_PER_VOICE]) {
            let mut senders: [Option<Animator>; ANIMATORS_PER_VOICE] =
                [const { None }; ANIMATORS_PER_VOICE];
            let mut receivers: [Option<_>; ANIMATORS_PER_VOICE] =
                [const { None }; ANIMATORS_PER_VOICE];
            for (index, channel) in channels.into_iter().enumerate() {
                let (sender, receiver) = channel.split();
                senders[index] = Some(sender);
                receivers[index] = Some(receiver);
            }
            let mut senders = senders.into_iter().map(Option::unwrap);
            let mut engines: [Option<AnimationEngine>; ANIMATORS_PER_VOICE] =
                [const { None }; ANIMATORS_PER_VOICE];
            for (index, receiver) in receivers.into_iter().enumerate() {
                engines[index] = Some(AnimationEngine::new(receiver.expect("animator slot")));
            }
            (
                VoiceAnimators {
                    $( $field: take_slots(&mut senders), )*
                },
                engines.map(Option::unwrap),
            )
        }

        $(
            paste::paste! {
                #[derive(Clone, Copy, Debug, PartialEq, Eq)]
                pub enum [<$variant Control>] {
                    $( $control, )*
                }

                impl $consumer {
                    pub(crate) const CONTROL_MAP:
                        [Option<[<$variant Control>]>; CONTROL_SLOTS] =
                        build_control_map(&[
                            $( ($control_slot, [<$variant Control>]::$control) ),*
                        ]);

                    pub(crate) fn control_for_slot(slot: u8) -> Option<[<$variant Control>]> {
                        Self::CONTROL_MAP.get(slot as usize).copied().flatten()
                    }
                }
            }
        )*

        const _: () = {
            let mut expected = 0usize;
            $( $( { assert!($slot == expected); expected += 1; } )* )*
            assert!(expected == ANIMATORS_PER_VOICE);
        };
    };
}

// The registry. Adding an animator-backed modulation starts and ends here.
modulation! {
    Glide {
        animation: Glide,
        consumer: Glider,
        settings: super::consumers::glide::settings::GlideState,
        animator: glide,
        slots: [0],
        controls: [
            (0, Enabled),
            (1, NoteOnVelocity),
            (2, BendRange),
            (3, Duration),
        ],
    }
    Envelope {
        animation: Envelope,
        consumer: ModTrigger,
        settings: super::consumers::envelope::settings::EnvelopeState,
        animator: envelopes,
        slots: [1, 2],
        controls: [
            (0, ReleaseDuration),
            (1, Sustain),
            (2, DecayDuration),
            (3, AttackDuration),
            (4, ReleaseCurve),
            (5, DecayCurve),
            (6, AttackCurve),
        ],
    }
}

#[enum_dispatch]
pub trait Modulation {
    fn update(&mut self, settings: &Settings);
    async fn duration_ms(&self) -> u32;
    async fn animate(&mut self, progress: f32, depth: f32, offset: f32) -> Messages;
    async fn reset(&mut self) -> Messages;
    async fn next_stage(&mut self) -> bool;
}
