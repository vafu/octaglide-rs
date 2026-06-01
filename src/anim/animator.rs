use futures::FutureExt;
use log::info;
use midi_msg::{Channel, ChannelVoiceMsg, ControlChange, MidiMsg};
use rtic_monotonics::{
    Monotonic,
    systick::{ExtU32, Systick, fugit::Instant},
};
use rtic_sync::channel::Receiver;

use crate::anim::modulators::{Messages, Modulator};

use super::modulators::Modulation;

#[derive(Debug)]
pub enum Cmd {
    Start(Modulator),
    Stop,
}

#[derive(Debug)]
pub struct AnimationEngine {
    rx: Receiver<'static, Cmd, 1>,
    looping: bool,
    state: State,
    _depth: f32,
    modulator: Option<Modulator>,
    last_continuous: heapless::Vec<MidiMsg, DEDUPE_SLOTS>,
}

#[derive(Debug)]
enum State {
    Idle,
    Animating {
        last_updated: Instant<u32, 1, 1000>,
        progress_at: f32,
    },
}

const MSG_INTERVAL_MS: u32 = 5;
const DEDUPE_SLOTS: usize = 4;

impl AnimationEngine {
    pub fn new(rx: Receiver<'static, Cmd, 1>) -> Self {
        AnimationEngine {
            rx,
            looping: false,
            state: State::Idle,
            _depth: 1.0,
            modulator: None,
            last_continuous: heapless::Vec::new(),
        }
    }

    pub async fn tick(&mut self) -> Messages {
        match self.state {
            State::Idle => self.recv_cmd().await,

            State::Animating {
                last_updated,
                progress_at: progress,
            } => {
                let Some(modulator) = self.modulator.as_mut() else {
                    return None;
                };
                let duration = modulator.duration_ms();

                futures::select_biased! {
                    new_msg = self.recv_cmd().fuse() => new_msg,
                    _ = Systick::delay(MSG_INTERVAL_MS.millis()).fuse() => {
                        let now = Systick::now();
                        let elapsed = now - last_updated;
                        let progress_delta = if duration == 0 { 1.0 } else { elapsed.to_millis() as f32 / duration as f32 };
                        let new_progress = (progress + progress_delta).clamp(0.0, 1.0);
                        let res = self.modulator.as_mut()?.animate(new_progress, 1.0, 1.0);
                        if new_progress >= 1.0 {
                            self.state = if self.modulator.as_mut()?.next_stage() || self.looping {
                                State::Animating { last_updated: now, progress_at: 0.0 }
                            } else {
                                info!("anim end");
                                State::Idle
                            };
                        } else {
                            self.state = State::Animating {
                                last_updated: now,
                                progress_at: new_progress,
                            }
                        }
                        self.dedupe_messages(res)
                    }
                }
            }
        }
    }

    fn dedupe_messages(&mut self, messages: Messages) -> Messages {
        let mut output = heapless::Vec::new();

        for msg in messages? {
            if self.is_duplicate_continuous(&msg) {
                continue;
            }
            let _ = output.push(msg);
        }

        if output.is_empty() {
            None
        } else {
            Some(output)
        }
    }

    fn is_duplicate_continuous(&mut self, msg: &MidiMsg) -> bool {
        let Some(pos) = self
            .last_continuous
            .iter()
            .position(|last| same_continuous_target(last, msg))
        else {
            if is_continuous(msg) {
                if self.last_continuous.push(msg.clone()).is_err() {
                    self.last_continuous.remove(0);
                    let _ = self.last_continuous.push(msg.clone());
                }
            }
            return false;
        };

        if self.last_continuous[pos] == *msg {
            true
        } else {
            self.last_continuous[pos] = msg.clone();
            false
        }
    }

    async fn recv_cmd(&mut self) -> Messages {
        let Ok(cmd) = self.rx.recv().await else {
            log::error!("error receiving Animator cmd");
            return None;
        };
        match cmd {
            Cmd::Start(modulator) => {
                self.last_continuous.clear();
                let mut messages = if let Some(old_mod) = self.modulator.as_mut() {
                    // resetting at start causes 2 identical messages (with 0 "reset" value) sent.
                    old_mod.reset().unwrap_or(heapless::Vec::new())
                } else {
                    heapless::Vec::new()
                };

                self.state = State::Animating {
                    last_updated: Systick::now(),
                    progress_at: 0.0,
                };
                self.modulator = Some(modulator);

                if let Some(new_msgs) = self.modulator.as_mut()?.animate(0.0, 0.0, 0.0) {
                    for msg in new_msgs {
                        let _ = messages.push(msg);
                    }
                }

                self.dedupe_messages(Some(messages))
            }
            Cmd::Stop => {
                let messages = match self.state {
                    State::Animating { .. } => {
                        self.state = State::Idle;
                        self.modulator.as_mut()?.reset()
                    }
                    State::Idle => self.modulator.as_mut()?.reset(),
                };
                let messages = self.dedupe_messages(messages);
                self.last_continuous.clear();
                messages
            }
        }
    }
}

fn is_continuous(msg: &MidiMsg) -> bool {
    let Some((_, voice)) = channel_voice(msg) else {
        return false;
    };

    matches!(
        voice,
        ChannelVoiceMsg::ControlChange {
            control: ControlChange::CC { .. } | ControlChange::CCHighRes { .. }
        } | ChannelVoiceMsg::PitchBend { .. }
    )
}

fn same_continuous_target(a: &MidiMsg, b: &MidiMsg) -> bool {
    let (Some((a_channel, a_voice)), Some((b_channel, b_voice))) =
        (channel_voice(a), channel_voice(b))
    else {
        return false;
    };

    if a_channel != b_channel {
        return false;
    }

    match (a_voice, b_voice) {
        (
            ChannelVoiceMsg::ControlChange { control: a_control },
            ChannelVoiceMsg::ControlChange { control: b_control },
        ) => same_control_target(a_control, b_control),
        (ChannelVoiceMsg::PitchBend { .. }, ChannelVoiceMsg::PitchBend { .. }) => true,
        _ => false,
    }
}

fn channel_voice(msg: &MidiMsg) -> Option<(Channel, &ChannelVoiceMsg)> {
    match msg {
        MidiMsg::ChannelVoice { channel, msg } | MidiMsg::RunningChannelVoice { channel, msg } => {
            Some((*channel, msg))
        }
        _ => None,
    }
}

fn same_control_target(a: &ControlChange, b: &ControlChange) -> bool {
    match (a, b) {
        (ControlChange::CC { control: a, .. }, ControlChange::CC { control: b, .. }) => a == b,
        (
            ControlChange::CCHighRes {
                control1: a1,
                control2: a2,
                ..
            },
            ControlChange::CCHighRes {
                control1: b1,
                control2: b2,
                ..
            },
        ) => a1 == b1 && a2 == b2,
        _ => false,
    }
}
