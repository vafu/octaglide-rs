# Modulation lane modes, generators and playback

Status: design draft from the 2026-11-01 discussion. No firmware changes are
included. Implementation task: `octaglide-modulation-generators` in `proj`.

## Goal

Each voice keeps four modulation lanes. A lane can forward modulation supplied
by Octatrack, or generate modulation inside Octaglide. For internal modulation,
Octatrack CC controls the amount of the generated signal. The generator's shape
and the way it is triggered must be independent choices: an envelope can loop,
and an LFO can play once.

“Lane” means one of the four output slots. “Lane mode” describes its source;
it does not mean an envelope's AD, AR or ADSR shape.

## Independent choices

| Choice | Options | Responsibility |
| --- | --- | --- |
| Lane mode | Passthrough, Internal | Where the output modulation comes from |
| Generator, for Internal | Envelope; later LFO and other shapes | How values evolve through a cycle |
| Playback, for Internal | Note-triggered one-shot, clock-driven loop; future policies | When a cycle starts, repeats and stops |
| Input mapping | Incoming channel/CC associated with the lane | Which external value is forwarded or used as amount |
| Output mapping | Voice output channel and lane CC destination | Where the resulting modulation is sent |

This is a responsibility split, not a requirement for a separate Rust trait or
task for each row. Concrete types and runtime resources need a later architecture
review. Avoid creating a separate consumer implementation for every combination.

## Lane modes

### Passthrough

The mapped Octatrack CC supplies the output value directly. The lane has no
generator or playback state. Whether input and output CC numbers are identical
or remapped belongs to the mapping contract and is not fixed by this draft.

The current `Consumers::Passthrough` is an inert placeholder: Voice forwards
remaining MIDI. It does not yet implement explicit per-lane CC mapping.

### Internal

The lane generates its own signal. Its mapped incoming CC becomes modulation
amount rather than a competing raw value at the output destination.

For an initially unipolar generator, the proposed relationship is:

```text
output = generator_level × amount
generator_level, amount, output are normalized to [0, 1]
```

Amount zero produces zero output; full amount preserves the generator's full
range. Changing amount should scale an already running generator without
restarting its phase or envelope. These are proposed semantics for review.
Initial amount is also a decision: full amount would preserve today's envelope
behavior before the first amount CC arrives.

Keep calculation precision independent of MIDI encoding. Do not quantize the
generator to seven bits before scaling: the planned high-resolution CC work
must retain fine changes. Bipolar LFO range and offset behavior remain open.

## Generators and playback

An envelope supplies its stages, durations, levels and curves. An LFO supplies
its waveform and cycle parameters. Playback determines when either is started
and repeated. Existing envelope shapes AD, AR and ADSR remain generator
configuration, separate from lane mode and playback policy.

| Generator | Note-triggered one-shot | Clock-driven loop |
| --- | --- | --- |
| Envelope | NoteOn starts one envelope traversal | Clock enables repeated envelope cycles |
| LFO | NoteOn starts one waveform cycle | Clock enables repeated waveform cycles |

The requested direction supports both columns for both generator types. LFO is
a future generator; this task does not assume it already exists.

The discussion called automatic cycling after clock detection “freeform.” Use
“clock-driven loop” as a working name to avoid implying an unsynchronized LFO.
Do not assume every MIDI clock pulse restarts the generator. The exact clock
relationship still needs agreement.

The other requested behavior is note-triggered one-shot. “Togglable” in the
discussion is captured here as a one-shot trigger, not an on/off latch. Future
policies can be added without redefining what an envelope or LFO is.

## Proposed ownership and signal flow

```text
Core input / UI interception
  → track and voice routing
    → voicing processors
      → four modulation lanes
        Passthrough: mapped CC → output mapping
        Internal: trigger/clock → generator → amount scaling → output mapping
```

Keep existing tracks and voice IDs, voice output channels, voicing-before-lanes
ordering and per-lane selection. Consumer-owned configuration and the separate
`UiProvider` contract remain useful. A lane's UI can expose mode, generator,
playback and amount without mixing navigation state into generator logic.

`ModTrigger` currently combines NoteOn handling with construction of an envelope
animation. It is the natural migration point: separate triggering/repetition
from envelope-specific behavior so the playback policy can drive either shape.
Generators must retain independent playback state per voice and lane.

Mapped amount CC must update its intended internal lane and suppress the raw CC
when it would overwrite generated output. Unrelated MIDI must keep its existing
forwarding behavior. Define mapping ownership before implementing this: a broad
CC match in an earlier lane could otherwise hide a value from a later lane.

MIDI clock is not channel-addressed. Core currently forwards system messages
without running voice consumers, so clock distribution to active internal lanes
needs an explicit path. Its relationship to track assignment and voice sharing
must not depend on a selected UI page.

## Decisions required before implementation

1. **CC mappings:** Which main/aux channel and CC controls each lane? Is amount
   exclusively external, or also editable locally? Are mappings shared or unique?
2. **Clock and transport:** Does the first clock pulse start looping, or is MIDI
   Start required? Does Stop halt, finish the current cycle, hold, or reset output?
   What happens on Continue, clock loss and tempo changes?
3. **Loop timing:** Does clock only start cycles that use local durations/rates,
   or synchronize cycle length to a beat division? What are the phase/reset rules?
4. **Envelope holds:** AD naturally completes. AR and ADSR currently wait for held
   notes. Define hold duration or gate behavior in one-shot and loop playback so
   a looping envelope cannot wait forever. NoteOff behavior also needs agreement.
5. **Retriggering:** Restart, continue, or ignore when a NoteOn arrives during a
   cycle? How do legato, multiple held notes and future shared voices behave?
6. **Live reconfiguration:** On lane mode, generator or playback changes, how is
   the old engine stopped and its last output cleared? Which settings are retained?
7. **Ranges and resource bounds:** Define unipolar/bipolar LFO behavior, startup
   amount, output deduplication after scaling, and bounded engine/channel storage
   when any of the four slots can become an internal generator.

## Delivery and verification

First review these semantics. Then agree the Rust ownership/API changes and the
clock path before implementing. Keep platform, transport and UI policy outside
portable generator math; preserve `no_std` and bounded storage in timing paths.

Tests should cover direct passthrough, amount zero/full/intermediate values and
live changes, independent lanes, suppression of owned amount CC, unrelated MIDI
forwarding, one-shot completion, loop timing and transport transitions, retrigger
rules, envelope hold/release behavior and safe lane replacement. Validate firmware
and release builds, then record actual board behavior separately.

Related tasks: `octaglide-modulation-lanes` provides the four-slot structure;
`octaglide-task-20` concerns broader musical note-trigger modes and should share
timing concepts where useful without being silently replaced by this lane design;
`octaglide-hires-envelopes` owns receiver-compatible high-resolution encoding;
`octaglide-ui-framework` owns navigation and model composition.
