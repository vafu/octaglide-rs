//! Host verification of portable semantic-core pieces.
#![allow(dead_code)]

#[path = "../../../src/app_event.rs"]
mod app_event;
#[path = "../../../src/calc/mod.rs"]
mod calc;
#[path = "../../../src/core/mod.rs"]
mod core;
#[path = "../../../src/midi/fmt.rs"]
mod midi;
#[path = "../../../src/notes/mod.rs"]
mod notes;
#[path = "../../../src/output/mod.rs"]
mod output;
#[path = "../../../src/state/mod.rs"]
mod state;
#[path = "../../../src/ui/controller.rs"]
mod controller;
#[path = "../../../src/ui/snapshot.rs"]
mod snapshot;

mod app {
    use rtic_sync::channel::Sender;
    pub type Animator = Sender<'static, crate::core::Cmd, 1>;
}

mod ui {
    pub use crate::controller::{Controller, UiEvent};
    pub use crate::snapshot::model;

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct OctaveUi { pub shift: i8 }
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct ModTriggerUi {
        pub name: &'static str,
        pub output_cc: u8,
        pub attack: EnvelopeStageUi,
        pub decay: EnvelopeStageUi,
        pub sustain: u8,
        pub release: EnvelopeStageUi,
        pub mode: u8,
    }
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct GliderUi {
        pub enabled: bool,
        pub duration_ms: u32,
        pub bend_range_semitones: f32,
        pub note_on_velocity: u8,
    }
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum UiModel { Octave(OctaveUi), ModTrigger(ModTriggerUi), Glider(GliderUi) }
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct EnvelopeStageUi { pub duration: u8, pub curve: u8 }
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct DisplaySnapshot {
        pub track: u8,
        pub voice: u8,
        pub lane: u8,
        pub usb_connected: bool,
        pub model: Option<UiModel>,
    }
    pub type UiWriter = rtic_sync::signal::SignalWriter<'static, DisplaySnapshot>;
}

#[cfg(test)]
#[path = "../../../src/ui/test.rs"]
mod ui_test;
