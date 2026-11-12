//! Events entering the application runtime.

use midi_msg::MidiMsg;
use rtic_sync::channel::{Receiver, Sender};

use crate::ui::UiEvent;

pub const APP_EVENT_CAPACITY: usize = 16;

#[derive(Clone, Debug, PartialEq)]
pub enum AppEvent {
    Midi(MidiMsg),
    Ui(UiEvent),
}

pub type AppSender = Sender<'static, AppEvent, APP_EVENT_CAPACITY>;
pub type AppReceiver = Receiver<'static, AppEvent, APP_EVENT_CAPACITY>;
