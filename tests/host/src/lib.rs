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
#[path = "../../../src/ui/mod.rs"]
mod ui;

mod app {
    use rtic_sync::channel::Sender;
    pub type Animator = Sender<'static, crate::core::Cmd, 1>;
}
