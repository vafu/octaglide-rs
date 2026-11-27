//! Single-owner storage for the runtime's bounded channels.

use rtic_sync::{
    channel::Channel,
    signal::Signal,
};

use crate::{
    app_event::{AppEvent, APP_EVENT_CAPACITY},
    core::{
        split_animator_slots, AnimationEngine, Cmd, VoiceAnimators, ANIMATORS_PER_VOICE,
        MAX_VOICES,
    },
    output::{OutputMessage, OUTPUT_CHANNEL_CAPACITY},
    ui::{ButtonEdge, DisplaySnapshot, BUTTON_QUEUE_CAPACITY},
};

/// Owns the channels shared by every platform runtime task.
///
/// Construct once, then call [`RuntimeStorage::split`] to hand out endpoints.
pub struct RuntimeStorage {
    pub output: Channel<OutputMessage, OUTPUT_CHANNEL_CAPACITY>,
    pub app: Channel<AppEvent, APP_EVENT_CAPACITY>,
    pub buttons: Channel<ButtonEdge, BUTTON_QUEUE_CAPACITY>,
    pub ui: Signal<DisplaySnapshot>,
    /// One command channel per (voice, animator). Split into senders for
    /// `Core` and receivers for the animation engines.
    pub animator_cmds: [[Channel<Cmd, 1>; ANIMATORS_PER_VOICE]; MAX_VOICES],
}

/// Split endpoints for a [`RuntimeStorage`].
pub struct RuntimeHandles {
    pub output_sender: crate::output::OutputSender,
    pub output_receiver: crate::output::OutputReceiver,
    pub app_sender: crate::app_event::AppSender,
    pub app_receiver: crate::app_event::AppReceiver,
    pub button_sender: crate::ui::ButtonSender,
    pub button_receiver: crate::ui::ButtonReceiver,
    pub ui_writer: rtic_sync::signal::SignalWriter<'static, DisplaySnapshot>,
    pub ui_reader: rtic_sync::signal::SignalReader<'static, DisplaySnapshot>,
    /// Senders handed to `Core`, so consumers can drive their modulators.
    pub animators: [VoiceAnimators; MAX_VOICES],
    /// Receivers driven by the animation task.
    pub engines: [[AnimationEngine; ANIMATORS_PER_VOICE]; MAX_VOICES],
}

impl RuntimeStorage {
    /// Creates empty channels. `const` so the value can live in a `StaticCell`.
    pub const fn new() -> Self {
        Self {
            output: Channel::new(),
            app: Channel::new(),
            buttons: Channel::new(),
            ui: Signal::new(),
            animator_cmds: [const { [const { Channel::new() }; ANIMATORS_PER_VOICE] }; MAX_VOICES],
        }
    }

    /// Splits every channel exactly once.
    ///
    /// The `&'static mut` receiver makes a second split impossible at compile time,
    /// matching the one-shot guarantee `make_channel!` enforces at runtime.
    pub fn split(&'static mut self) -> RuntimeHandles {
        let (output_sender, output_receiver) = self.output.split();
        let (app_sender, app_receiver) = self.app.split();
        let (button_sender, button_receiver) = self.buttons.split();
        let (ui_writer, ui_reader) = self.ui.split();

        let mut animators: [Option<VoiceAnimators>; MAX_VOICES] = [const { None }; MAX_VOICES];
        let mut engines: [Option<[AnimationEngine; ANIMATORS_PER_VOICE]>; MAX_VOICES] =
            [const { None }; MAX_VOICES];

        for (index, channels) in self.animator_cmds.iter_mut().enumerate() {
            let (senders, voice_engines) = split_animator_slots(channels);
            animators[index] = Some(senders);
            engines[index] = Some(voice_engines);
        }

        RuntimeHandles {
            output_sender,
            output_receiver,
            app_sender,
            app_receiver,
            button_sender,
            button_receiver,
            ui_writer,
            ui_reader,
            animators: animators.map(Option::unwrap),
            engines: engines.map(Option::unwrap),
        }
    }
}
