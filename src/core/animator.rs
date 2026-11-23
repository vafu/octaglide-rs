use futures::FutureExt;
use log::debug;
use rtic_monotonics::{Monotonic, systick::{Systick, fugit::Instant}};
#[cfg(target_os = "none")]
use rtic_monotonics::systick::ExtU32;
use rtic_sync::channel::Receiver;

use super::{Messages, Modulation, Modulator, VoiceOutput};

#[derive(Debug)]
pub struct AnimationEngine {
    rx: Receiver<'static, Cmd, 1>,
    looping: bool,
    state: State,
    _depth: f32,
    modulator: Option<Modulator>,
    last_continuous: heapless::Vec<VoiceOutput, DEDUPE_SLOTS>,
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
                let duration = modulator.duration_ms().await;

                futures::select_biased! {
                    new_msg = self.recv_cmd().fuse() => new_msg,
                    _ = animation_delay().fuse() => {
                        let now = Systick::now();
                        let elapsed = now - last_updated;
                        let progress_delta = if duration == 0 { 1.0 } else { elapsed.to_millis() as f32 / duration as f32 };
                        let new_progress = (progress + progress_delta).clamp(0.0, 1.0);
                        let res = self.modulator.as_mut()?.animate(new_progress, 1.0, 1.0).await;
                        if new_progress >= 1.0 {
                            self.state = if self.modulator.as_mut()?.next_stage().await || self.looping {
                                State::Animating { last_updated: now, progress_at: 0.0 }
                            } else {
                                debug!("anim end");
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
        if output.is_empty() { None } else { Some(output) }
    }

    fn is_duplicate_continuous(&mut self, msg: &VoiceOutput) -> bool {
        let Some(pos) = self
            .last_continuous
            .iter()
            .position(|last| same_continuous_target(last, msg))
        else {
            if is_continuous(msg) {
                if self.last_continuous.push(*msg).is_err() {
                    self.last_continuous.remove(0);
                    let _ = self.last_continuous.push(*msg);
                }
            }
            return false;
        };
        if self.last_continuous[pos] == *msg {
            true
        } else {
            self.last_continuous[pos] = *msg;
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
                    old_mod.reset().await.unwrap_or(heapless::Vec::new())
                } else {
                    heapless::Vec::new()
                };
                self.state = State::Animating {
                    last_updated: Systick::now(),
                    progress_at: 0.0,
                };
                self.modulator = Some(modulator);
                if let Some(new_msgs) = self.modulator.as_mut()?.animate(0.0, 0.0, 0.0).await {
                    for msg in new_msgs {
                        let _ = messages.push(msg);
                    }
                }
                self.dedupe_messages(Some(messages))
            }
            Cmd::Update(settings) => {
                if let Some(modulator) = self.modulator.as_mut() {
                    modulator.update(&settings);
                }
                None
            }
            Cmd::Stop => {
                let messages = match self.state {
                    State::Animating { .. } => {
                        self.state = State::Idle;
                        self.modulator.as_mut()?.reset().await
                    }
                    State::Idle => self.modulator.as_mut()?.reset().await,
                };
                let messages = self.dedupe_messages(messages);
                self.last_continuous.clear();
                messages
            }
        }
    }
}

async fn animation_delay() {
    #[cfg(target_os = "none")]
    Systick::delay(MSG_INTERVAL_MS.millis()).await;
    #[cfg(not(target_os = "none"))]
    core::future::pending::<()>().await;
}

fn is_continuous(msg: &VoiceOutput) -> bool {
    matches!(msg, VoiceOutput::Control { .. } | VoiceOutput::PitchBend { .. })
}

fn same_continuous_target(a: &VoiceOutput, b: &VoiceOutput) -> bool {
    match (a, b) {
        (
            VoiceOutput::Control { voice: a_voice, slot: a_slot, .. },
            VoiceOutput::Control { voice: b_voice, slot: b_slot, .. },
        ) => a_voice == b_voice && a_slot == b_slot,
        (
            VoiceOutput::PitchBend { voice: a_voice, .. },
            VoiceOutput::PitchBend { voice: b_voice, .. },
        ) => a_voice == b_voice,
        _ => false,
    }
}

/// One glide engine and two envelope engines per voice.
pub const ANIMATORS_PER_VOICE: usize = 3;

pub type Animator = rtic_sync::channel::Sender<'static, Cmd, 1>;

#[derive(Debug)]
pub struct VoiceAnimators {
    pub glide: Animator,
    pub envelopes: [Animator; 2],
}

#[derive(Debug)]
pub enum Cmd {
    Start(Modulator),
    Update(Settings),
    Stop,
}

#[derive(Clone, Copy, Debug)]
pub enum Settings {
    Envelope(crate::state::EnvelopeState),
    Glide(crate::state::GlideState),
}
