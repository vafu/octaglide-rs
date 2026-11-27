use super::{Consumer, Input, ModLanes, OutputEvents, Voice, VoiceOutput, Voicing};

impl Voice {
    /// Runs the full chain: voicing transforms, then modulation lanes.
    pub(super) async fn process_input(&mut self, input: Input) -> OutputEvents {
        let voiced = run_voicing(&mut self.voicing, input).await;
        let mut out = OutputEvents::new();
        for msg in voiced {
            match to_input(msg) {
                Some(input) => {
                    for lane_msg in run_lanes(&mut self.modulations, input).await {
                        out.push(lane_msg).ok();
                    }
                }
                None => {
                    out.push(msg).ok();
                }
            }
        }
        out
    }

    /// Runs the modulation lanes from `lane` onward.
    pub(super) async fn process_lane(&mut self, lane: usize, input: Input) -> OutputEvents {
        let Some(chain) = self.modulations.get_mut(lane..) else {
            return OutputEvents::new();
        };
        run_lanes(chain, input).await
    }
}

async fn run_voicing(chain: &mut [Voicing], input: Input) -> OutputEvents {
    let mut current = OutputEvents::new();
    let Some((first, rest)) = chain.split_first_mut() else {
        if let Some(output) = input_to_output(input) {
            current.push(output).ok();
        }
        return current;
    };
    dispatch_consumer(first, input, &mut current).await;
    for consumer in rest {
        if current.is_empty() {
            break;
        }
        let mut next = OutputEvents::new();
        for msg in core::mem::take(&mut current) {
            match to_input(msg) {
                Some(input) => dispatch_consumer(consumer, input, &mut next).await,
                None => {
                    next.push(msg).ok();
                }
            }
        }
        current = next;
    }
    current
}

async fn run_lanes(chain: &mut [ModLanes], input: Input) -> OutputEvents {
    let mut current = OutputEvents::new();
    let Some((first, rest)) = chain.split_first_mut() else {
        if let Some(output) = input_to_output(input) {
            current.push(output).ok();
        }
        return current;
    };
    dispatch_consumer(first, input, &mut current).await;
    for consumer in rest {
        if current.is_empty() {
            break;
        }
        let mut next = OutputEvents::new();
        for msg in core::mem::take(&mut current) {
            match to_input(msg) {
                Some(input) => dispatch_consumer(consumer, input, &mut next).await,
                None => {
                    next.push(msg).ok();
                }
            }
        }
        current = next;
    }
    current
}

async fn dispatch_consumer<C: Consumer>(
    consumer: &mut C,
    input: Input,
    out: &mut OutputEvents,
) {
    let consumed = consumer.consume(&input, out).await;
    if !consumed && let Some(output) = input_to_output(input) {
        if out.push(output).is_err() {
            log::error!("Voice output capacity exceeded");
        }
    }
}

fn input_to_output(input: Input) -> Option<VoiceOutput> {
    match input {
        Input::NoteOn { voice, note, velocity } => Some(VoiceOutput::NoteOn { voice, note, velocity }),
        Input::NoteOff { voice, note, velocity } => Some(VoiceOutput::NoteOff { voice, note, velocity }),
        _ => None,
    }
}

fn to_input(output: VoiceOutput) -> Option<Input> {
    match output {
        VoiceOutput::NoteOn { voice, note, velocity } => Some(Input::NoteOn { voice, note, velocity }),
        VoiceOutput::NoteOff { voice, note, velocity } => Some(Input::NoteOff { voice, note, velocity }),
        _ => None,
    }
}
