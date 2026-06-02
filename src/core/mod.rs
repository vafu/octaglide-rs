mod consumers;
pub mod event;
mod routing;

use log::info;
use midi_msg::{ChannelVoiceMsg, MidiMsg};

use crate::midi::MidiFmt;
use crate::state::{self, EnvelopeParam, ModifierParam};

use self::routing::RoutedMidi;

#[derive(Debug)]
pub struct MidiOut {
    pub msg: MidiMsg,
    pub tag: &'static str,
}

use crate::{
    app::{Animator, MidiSender},
    core::{
        consumers::{Consumer, Consumers, Glider, ModTrigger, Octave, Passthrough},
        event::{Event, EventPayload, EventRole, Events, HardwareControl},
    },
};
use heapless::Vec;

/// What the encoder + sliders are currently controlling.
#[derive(Debug, Clone, Copy)]
enum EncoderMode {
    /// Sliders 0-3 → A/D/S/R durations.
    Time,
    /// Sliders 0,1,3 → A/D/R curve shapes; slider 2 still controls sustain level.
    Curve,
}

pub struct Core {
    consumers: Vec<Consumers, 8>,
    midi_sender: MidiSender,
    encoder_mode: EncoderMode,
}

#[derive(Debug)]
pub enum Input {
    Midi(MidiMsg),
    Hardware {
        control: HardwareControl,
        payload: EventPayload,
    },
}

macro_rules! vec {
    ($($item:expr),* $(,)?) => {{
        let mut v = Vec::new();
        $(v.push($item).unwrap();)*
        v
    }};
}

impl Core {
    pub fn new(
        midi_sender: MidiSender,
        glide_animator: Animator,
        envelope_animator: Animator,
    ) -> Self {
        let consumers = vec![
            Consumers::Octave(Octave::new()),
            Consumers::Glider(Glider::new(glide_animator)),
            Consumers::ModTrigger(ModTrigger::new(envelope_animator)),
            Consumers::Passthrough(Passthrough::new(midi_sender.clone())),
        ];

        Core {
            consumers,
            midi_sender,
            encoder_mode: EncoderMode::Time,
        }
    }

    pub async fn process(&mut self, input: Input) {
        match input {
            Input::Midi(msg) => match routing::route_midi(msg).await {
                RoutedMidi::Event(event) => {
                    self.dispatch_event(event).await;
                }
                RoutedMidi::Passthrough(msg) => self.send_raw(msg, "raw").await,
                RoutedMidi::Drop => {}
            },
            Input::Hardware { control, payload } => {
                self.process_hardware(control, payload).await;
            }
        }
    }

    async fn process_hardware(&mut self, control: HardwareControl, payload: EventPayload) {
        if control == HardwareControl::EncoderClick {
            self.encoder_mode = match self.encoder_mode {
                EncoderMode::Time => EncoderMode::Curve,
                EncoderMode::Curve => EncoderMode::Time,
            };
            info!("Encoder mode → {:?}", self.encoder_mode);
            return;
        }

        let Some(param) = self.hardware_param(control) else {
            return;
        };
        let voice = state::selected_voice().await;

        self.dispatch_event(Event {
            role: EventRole::Modifier { voice, param },
            payload,
        })
        .await;
    }

    fn hardware_param(&self, control: HardwareControl) -> Option<ModifierParam> {
        let param = match (self.encoder_mode, control) {
            (EncoderMode::Time, HardwareControl::Slider3) => EnvelopeParam::AttackDuration,
            (EncoderMode::Time, HardwareControl::Slider2) => EnvelopeParam::DecayDuration,
            (EncoderMode::Time, HardwareControl::Slider1) => EnvelopeParam::Sustain,
            (EncoderMode::Time, HardwareControl::Slider0) => EnvelopeParam::ReleaseDuration,
            (EncoderMode::Curve, HardwareControl::Slider3) => EnvelopeParam::AttackCurve,
            (EncoderMode::Curve, HardwareControl::Slider2) => EnvelopeParam::DecayCurve,
            (EncoderMode::Curve, HardwareControl::Slider1) => EnvelopeParam::Sustain,
            (EncoderMode::Curve, HardwareControl::Slider0) => EnvelopeParam::ReleaseCurve,
            (_, HardwareControl::Encoder) => EnvelopeParam::Mode,
            _ => return None,
        };

        Some(ModifierParam::Envelope(param))
    }

    async fn dispatch_event(&mut self, event: Event) {
        let mut current = Events::new();
        current.push(event).ok();

        for consumer in &mut self.consumers {
            if current.is_empty() {
                break;
            }

            let mut next = Events::new();
            for event in current {
                consumer.consume(event, &mut next).await;
            }
            current = next;
        }
    }

    async fn send_raw(&mut self, msg: MidiMsg, tag: &'static str) {
        log_msg("<<<", &msg);
        self.midi_sender.send(MidiOut { msg, tag }).await.ok();
    }
}

fn log_msg(prefix: &str, msg: &MidiMsg) {
    // Skip logging PitchBend to avoid noise
    if !matches!(
        msg,
        MidiMsg::ChannelVoice {
            msg: ChannelVoiceMsg::PitchBend { .. },
            ..
        }
    ) {
        info!("{} {}", prefix, MidiFmt(msg));
    }
}
