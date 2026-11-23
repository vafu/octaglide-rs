//! Host verification of portable semantic-core pieces.
#![allow(dead_code)]

#[path = "../../../src/app_event.rs"]
mod app_event;
#[path = "../../../src/calc/mod.rs"]
mod calc;
#[path = "../../../src/core/mod.rs"]
mod core;
#[path = "../../../src/midi/fmt.rs"]
mod midi_fmt;
#[path = "../../../src/midi/controller.rs"]
mod midi_controller;

#[path = "../../../src/midi/bus.rs"]
mod midi_bus;

mod midi {
    pub use crate::midi_bus::MidiBus;
    pub use crate::midi_controller::MidiController;
    pub use crate::midi_fmt::*;
}
#[path = "../../../src/notes/mod.rs"]
mod notes;
#[path = "../../../src/output/mod.rs"]
mod output;
#[path = "../../../src/state/mod.rs"]
mod state;
#[path = "../../../src/runtime/mod.rs"]
mod runtime;
#[path = "../../../src/ui/mod.rs"]
mod ui;

mod app {
    use rtic_sync::channel::Sender;
    pub type Animator = Sender<'static, crate::core::Cmd, 1>;
}

#[cfg(test)]
mod storage_test;

#[cfg(test)]
mod core_task_test;
