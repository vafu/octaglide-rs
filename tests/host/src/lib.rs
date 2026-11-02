//! Host verification of the real portable Core, consumers, models and modulators.
//! Only RTIC app channel aliases are supplied here; hardware tasks are excluded.
#![allow(dead_code)]

#[path = "../../../src/anim/mod.rs"]
mod anim;
#[path = "../../../src/core/mod.rs"]
mod core;
#[path = "../../../src/midi/fmt.rs"]
mod midi;
#[path = "../../../src/notes/mod.rs"]
mod notes;
#[path = "../../../src/state/mod.rs"]
mod state;
#[path = "../../../src/ui/mod.rs"]
mod ui;

mod app {
    use rtic_sync::channel::Sender;
    pub type MidiSender = Sender<'static, crate::core::MidiOut, 16>;
    pub type Animator = Sender<'static, crate::anim::Cmd, 1>;
}
