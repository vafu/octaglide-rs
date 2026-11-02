use midi_msg::MidiMsg;

use super::{
    Input, MidiOut, OutputEvents, Voice,
    consumers::{Consumer, Consumers},
};

impl Voice {
    /// Musical MIDI always runs voicing processors before all modulation lanes.
    pub(super) async fn process_midi(&mut self, msg: MidiMsg) {
        let chain = self.voicing.iter_mut().chain(self.modulations.iter_mut());
        for msg in run_chain(chain, Input::Midi(msg)).await {
            self.send_midi(msg).await;
        }
    }

    /// Hardware edits target one modulation lane; generated MIDI continues onward.
    pub(super) async fn process_lane(&mut self, lane: usize, input: Input) {
        let Some(chain) = self.modulations.get_mut(lane..) else {
            return;
        };
        for msg in run_chain(chain.iter_mut(), input).await {
            self.send_midi(msg).await;
        }
    }

    async fn send_midi(&mut self, msg: MidiMsg) {
        let msg = match msg {
            MidiMsg::ChannelVoice { msg, .. } | MidiMsg::RunningChannelVoice { msg, .. } => {
                MidiMsg::ChannelVoice {
                    channel: self.output_channel,
                    msg,
                }
            }
            MidiMsg::ChannelMode { msg, .. } | MidiMsg::RunningChannelMode { msg, .. } => {
                MidiMsg::ChannelMode {
                    channel: self.output_channel,
                    msg,
                }
            }
            other => other,
        };
        self.midi_sender
            .send(MidiOut { msg, tag: "voice" })
            .await
            .ok();
    }
}

async fn run_chain<'a>(
    mut chain: impl Iterator<Item = &'a mut Consumers>,
    input: Input,
) -> OutputEvents {
    let mut current = OutputEvents::new();
    if let Some(first) = chain.next() {
        dispatch_consumer(first, input, &mut current).await;
        let mut next = OutputEvents::new();
        for consumer in chain {
            if current.is_empty() {
                break;
            }
            for msg in core::mem::take(&mut current) {
                dispatch_consumer(consumer, Input::Midi(msg), &mut next).await;
            }
            core::mem::swap(&mut current, &mut next);
        }
    } else if let Input::Midi(msg) = input {
        current.push(msg).ok();
    }
    current
}

/// Keep generated messages, then append the original MIDI if it was not consumed.
/// The borrowed input lets a consumer inspect it without cloning it just to pass.
pub(super) async fn dispatch_consumer<C: Consumer>(
    consumer: &mut C,
    input: Input,
    out: &mut OutputEvents,
) {
    let consumed = consumer.consume(&input, out).await;
    if !consumed && let Input::Midi(msg) = input {
        if out.push(msg).is_err() {
            log::error!("Voice MIDI output capacity exceeded");
        }
    }
}
