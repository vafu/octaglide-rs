//! Host verification of portable semantic-core pieces.
#![allow(dead_code)]

#[path = "../../../src/app_event.rs"]
mod app_event;
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
#[path = "../../../src/host/input.rs"]
mod host_input;


#[cfg(test)]
mod channels_test;

#[cfg(test)]
mod core_task_test;

#[cfg(test)]
mod buttons_test;

#[cfg(test)]
mod renderer_test;

#[cfg(test)]
mod host_input_test;

#[cfg(test)]
mod boundary_test;

#[cfg(test)]
mod host_loop_test;
