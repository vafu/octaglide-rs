# Runtime Extraction and Host Simulator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the Teensy RTIC application onto a shared portable core and runtime, isolate every board-specific dependency under `src/teensy/`, and run the same task graph and OLED renderer on the host under a single-threaded Tokio runtime.

**Architecture:** `core` holds domain logic plus a `Clock` trait. `runtime` holds the shared async task bodies and `RuntimeStorage` for `rtic-sync` channels. `teensy` and `host` are thin platform shells that provide a clock, a MIDI bus, and a display. The host uses one `LocalSet` with one local task per logical component; it does not emulate RTIC priorities.

**Tech Stack:** Rust 2024, RTIC 2.2, `rtic-sync` 1.4, `rtic-monotonics`, `teensy4-bsp`, Tokio (current-thread + `LocalSet`), `embedded-graphics` 0.8, `embedded-graphics-simulator` 0.8, `static_cell` 2.

**Spec:** `docs/superpowers/specs/2026-11-23-runtime-extraction-design.md`

## Global Constraints

- Firmware target stays `thumbv7em-none-eabihf` via `.cargo/config.toml`.
- `core` and `runtime` must not import `teensy4_bsp`, `teensy_usbhost`, `cortex_m`, `rtic::app`, `rtic_monotonics`, or Tokio.
- `rtic-sync` stays the channel implementation on both platforms.
- `rtic-sync::Sender` is `Send` but not `Sync`: clone senders, never share `&Sender` across tasks.
- Never call `make_channel!` or `make_signal!` inside reusable functions; they use function-local statics and panic on a second call.
- Host builds use `--no-default-features --features host --target x86_64-unknown-linux-gnu`.
- Exactly one platform feature must be selected per build.
- The host does not reproduce RTIC preemption or priorities.
- No QEMU or Renode work.
- `src/core/consumers/mod_trigger.rs`, `src/core/consumers/glider.rs`, and `src/ui/navigation.rs` are undeclared orphan files. Do not edit them.
- Commits stay short and tagged, e.g. `[Refactor] ...` or `[Feature] ...`.

## Review Focus

- A MIDI note must produce the same `Core` output through the host path as through the firmware path.
- A held button must emit exactly one press and one release after debounce.
- The renderer must draw both passthrough and envelope snapshots without panicking.
- The renderer must ignore out-of-bounds drawing at 128×64.
- A full output queue must not stall UI processing; `run_midi_dispatch` must remain the sole drainer.

---

### Task 1: Portable Animator Alias

**Files:**
- Modify: `src/core/animator.rs:189-196`
- Modify: `src/core/mod.rs:14-16`
- Modify: `src/core/consumers/envelope/mod.rs:1-6`
- Modify: `src/core/consumers/glide/mod.rs:1-6`
- Modify: `src/core/test.rs:17-19`
- Modify: `src/main.rs:104`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub type Animator = rtic_sync::channel::Sender<'static, Cmd, 1>;` exported from `crate::core`.

- [ ] **Step 1: Add the alias**

Add next to `Cmd` in `src/core/animator.rs`:

```rust
pub type Animator = rtic_sync::channel::Sender<'static, Cmd, 1>;
```

Re-export it from `src/core/mod.rs` in the existing `animator` list.

- [ ] **Step 2: Replace `crate::app::Animator` in compiled core files**

Update `src/core/consumers/envelope/mod.rs`, `src/core/consumers/glide/mod.rs`, `src/core/test.rs`, and the `VoiceAnimators` fields in `src/core/animator.rs` to use `crate::core::Animator`. In `src/main.rs:104`, set `pub type Animator = crate::core::Animator;`.

- [ ] **Step 3: Verify**

Run: `cargo check`
Expected: PASS.

Run: `cargo test --manifest-path tests/host/Cargo.toml --target x86_64-unknown-linux-gnu --offline`
Expected: 14 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/core src/main.rs
git commit -m "[Refactor] Move animator sender alias into core"
```

---

### Task 2: Clock Trait and Portable Animation Timing

**Files:**
- Create: `src/core/clock.rs`
- Modify: `src/core/mod.rs:3-26`
- Modify: `src/core/animator.rs:1-30, 44-82, 117-170`
- Modify: `src/anim/runtime.rs`
- Modify: `src/anim/mod.rs`
- Modify: `src/main.rs:82, 518-525`
- Modify: `tests/host/src/lib.rs`
- Test: `src/core/test.rs`

**Interfaces:**
- Consumes: `core::Animator` from Task 1.
- Produces:
  - `pub trait Clock { fn now_ms() -> u32; async fn delay_ms(ms: u32); }`
  - `pub async fn AnimationEngine::tick<C: Clock>(&mut self) -> Messages`
  - `pub async fn run_animations<C: Clock>(engines: [[AnimationEngine; ANIMATORS_PER_VOICE]; MAX_VOICES], sender: OutputSender)`

- [ ] **Step 1: Write the failing test**

Add to `src/core/test.rs`:

```rust
struct TestClock;

impl crate::core::Clock for TestClock {
    fn now_ms() -> u32 { 0 }
    async fn delay_ms(_ms: u32) {}
}

#[test]
fn animation_engine_accepts_a_platform_clock() {
    let _guard = TEST_NOTES.lock().unwrap();
    block_on(async {
        let (sender, receiver) = channel();
        drop(sender);
        let mut engine = crate::core::AnimationEngine::new(receiver);
        assert!(engine.tick::<TestClock>().await.is_none());
    });
}
```

Dropping `sender` makes `recv_cmd` return immediately, so the `Idle` path completes.

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --manifest-path tests/host/Cargo.toml --target x86_64-unknown-linux-gnu --offline animation_engine_accepts_a_platform_clock`
Expected: FAIL — `Clock` and the generic `tick` do not exist.

- [ ] **Step 3: Add the trait and make timing generic**

Create `src/core/clock.rs` with the trait above and re-export it from `src/core/mod.rs`.

In `src/core/animator.rs`:

- Change `State::Animating.last_updated` to `u32`.
- Change `tick` and `recv_cmd` to take `C: Clock`.
- Use `C::now_ms()` and `C::now_ms().wrapping_sub(last_updated)`.
- Replace `animation_delay()` with `C::delay_ms(MSG_INTERVAL_MS).await` inside `select_biased!`.
- Delete `animation_delay` and all `rtic_monotonics` imports.

- [ ] **Step 4: Make the animation runtime generic**

In `src/anim/runtime.rs`, change `run_animations`, `run_voice`, and `run_engine` to take `C: Clock`, and call `engine.tick::<C>()`. Remove both `#[cfg(target_os = "none")]` gates from `src/anim/mod.rs`.

- [ ] **Step 5: Add the platform clocks**

In `tests/host/src/lib.rs`, remove the fake `app::Animator` module, add `#[path = "../../../src/core/clock.rs"] mod clock;`, and re-export `Clock` from the `core` shim.

In `src/main.rs`, add a temporary `SystickClock` inside the RTIC app:

```rust
pub struct SystickClock;
impl crate::core::Clock for SystickClock {
    fn now_ms() -> u32 {
        rtic_monotonics::systick::Systick::now()
            .duration_since_epoch()
            .to_millis()
    }
    async fn delay_ms(ms: u32) {
        use rtic_monotonics::systick::ExtU32;
        rtic_monotonics::systick::Systick::delay(ms.millis()).await
    }
}
```

Use it at the `run_animations` call site.

- [ ] **Step 6: Verify**

Run: `cargo check`
Expected: PASS.

Run: `cargo test --manifest-path tests/host/Cargo.toml --target x86_64-unknown-linux-gnu --offline`
Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/core src/anim src/main.rs tests/host/src/lib.rs
git commit -m "[Refactor] Decouple animation timing from Systick"
```

---

### Task 3: Ungate Portable UI Types

**Files:**
- Modify: `src/ui/mod.rs:1-52`
- Modify: `tests/host/src/lib.rs:28-64`

**Interfaces:**
- Consumes: nothing.
- Produces: `ButtonEdge`, `ButtonSender`, `ButtonReceiver`, `BUTTON_QUEUE_CAPACITY`, `ENCODER_BUTTON_INDEX`, `UiReader`, and `EncoderDecoder` available on every target.

- [ ] **Step 1: Remove the target gates**

In `src/ui/mod.rs`, delete the `#[cfg(target_os = "none")]` attributes on `encoder`, `envelope_plot`, `ButtonEdge`, `ENCODER_BUTTON_INDEX`, `BUTTON_QUEUE_CAPACITY`, `ButtonSender`, `ButtonReceiver`, and `UiReader`. Keep `display` and `hardware` gated for now.

- [ ] **Step 2: Use path includes in the host shim**

In `tests/host/src/lib.rs`, replace the hand-written `ui` module with:

```rust
#[path = "../../../src/ui/mod.rs"]
mod ui;
```

Keep `#[cfg(test)]` on the `ui/test.rs` include. Add any path modules that `ui/mod.rs` requires and that the shim does not already provide.

- [ ] **Step 3: Verify**

Run: `cargo test --manifest-path tests/host/Cargo.toml --target x86_64-unknown-linux-gnu --offline`
Expected: all existing tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/ui tests/host/src/lib.rs
git commit -m "[Refactor] Make portable UI types target-independent"
```

---

### Task 4: Runtime Storage

**Files:**
- Create: `src/runtime/mod.rs`
- Create: `src/runtime/storage.rs`
- Modify: `Cargo.toml`
- Modify: `src/main.rs:48-60`
- Modify: `tests/host/src/lib.rs`
- Modify: `tests/host/Cargo.toml`
- Test: `tests/host/src/storage_test.rs`

**Interfaces:**
- Consumes: `AppEvent`, `OutputMessage`, `ButtonEdge`, `DisplaySnapshot`, `Cmd`, `MAX_VOICES`, `ANIMATORS_PER_VOICE`, `AnimationEngine`, `VoiceAnimators`.
- Produces:
  - `pub struct RuntimeStorage { ... }`
  - `pub const fn RuntimeStorage::new() -> Self`
  - `pub fn RuntimeStorage::split(&'static mut self) -> RuntimeHandles`
  - `pub struct RuntimeHandles { ... }`

- [ ] **Step 1: Write the failing tests**

Add `static_cell = "2"` to `tests/host/Cargo.toml`. Add to `tests/host/src/lib.rs`:

```rust
#[path = "../../../src/runtime/mod.rs"]
mod runtime;

#[cfg(test)]
mod storage_test;
```

Create `tests/host/src/storage_test.rs`:

```rust
use crate::runtime::{RuntimeStorage, ANIMATORS_PER_VOICE, MAX_VOICES};

#[test]
fn split_produces_channels_and_engines_for_every_voice() {
    let storage: &'static mut RuntimeStorage = Box::leak(Box::new(RuntimeStorage::new()));
    let handles = storage.split();
    assert_eq!(handles.animators.len(), MAX_VOICES);
    assert_eq!(handles.animators[0].envelopes.len(), 2);
    assert_eq!(handles.engines.len(), MAX_VOICES);
    assert_eq!(handles.engines[0].len(), ANIMATORS_PER_VOICE);
}

#[test]
fn cloned_senders_reach_the_same_receiver() {
    let storage: &'static mut RuntimeStorage = Box::leak(Box::new(RuntimeStorage::new()));
    let mut handles = storage.split();
    let mut extra = handles.app_sender.clone();
    extra.try_send(crate::app_event::AppEvent::Ui(crate::ui::UiEvent::UsbConnection {
        connected: true,
    }))
    .unwrap();
    assert!(matches!(
        handles.app_receiver.try_recv(),
        Ok(crate::app_event::AppEvent::Ui(crate::ui::UiEvent::UsbConnection {
            connected: true
        }))
    ));
}

#[test]
fn signal_round_trips_a_snapshot() {
    let storage: &'static mut RuntimeStorage = Box::leak(Box::new(RuntimeStorage::new()));
    let mut handles = storage.split();
    let snapshot = crate::ui::DisplaySnapshot {
        track: 2,
        voice: 0,
        lane: 2,
        usb_connected: false,
        model: None,
    };
    handles.ui_writer.write(snapshot);
    assert_eq!(handles.ui_reader.try_read(), Some(snapshot));
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test --manifest-path tests/host/Cargo.toml --target x86_64-unknown-linux-gnu storage`
Expected: FAIL — `RuntimeStorage` does not exist.

- [ ] **Step 3: Implement storage and handles**

`RuntimeStorage` holds:

```rust
pub output: Channel<OutputMessage, 32>,
pub app: Channel<AppEvent, 16>,
pub buttons: Channel<ButtonEdge, 16>,
pub ui: Signal<DisplaySnapshot>,
pub animators: [[Channel<Cmd, 1>; ANIMATORS_PER_VOICE]; MAX_VOICES],
```

`RuntimeStorage::new()` is `const` and initializes the nested array with:

```rust
animators: [const { [const { Channel::new() }; ANIMATORS_PER_VOICE] }; MAX_VOICES],
```

`RuntimeHandles` exposes:

- `output_sender`, `output_receiver`
- `app_sender`, `app_receiver`
- `button_sender`, `button_receiver`
- `ui_writer`, `ui_reader`
- `animators: [VoiceAnimators; MAX_VOICES]`
- `engines: [[AnimationEngine; ANIMATORS_PER_VOICE]; MAX_VOICES]`

`split` takes `&'static mut self`, so it can only be called once per storage instance. Split the four top-level channels and the signal first. Then fill two `[Option<_>; MAX_VOICES]` arrays in a loop over `self.animators.iter_mut()`, destructuring each voice as `[glide, first, second]`. Convert with `animators.map(Option::unwrap)` and `engines.map(Option::unwrap)`.

Do not use `core::array::from_fn` here; it cannot borrow `self` mutably per element.

`src/runtime/mod.rs` must expose the names the tests and later tasks import:

```rust
mod storage;

pub use storage::{RuntimeHandles, RuntimeStorage};
pub use crate::core::{ANIMATORS_PER_VOICE, MAX_VOICES};
```

- [ ] **Step 4: Wire the module into both crates**

Add `mod runtime;` to `src/main.rs` and add `static_cell = "2"` to the firmware `Cargo.toml`.

- [ ] **Step 5: Verify**

Run: `cargo check`
Expected: PASS.

Run: `cargo test --manifest-path tests/host/Cargo.toml --target x86_64-unknown-linux-gnu`
Expected: all tests pass, including the three new storage tests.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/runtime src/main.rs tests/host
git commit -m "[Refactor] Add shared runtime storage"
```

---

### Task 5: Extract Core and MIDI Task Bodies

**Files:**
- Create: `src/runtime/core_task.rs`
- Create: `src/runtime/midi_dispatch.rs`
- Modify: `src/runtime/mod.rs`
- Modify: `src/main.rs:349-422`
- Test: `src/runtime/core_task.rs`

**Interfaces:**
- Consumes: `RuntimeHandles` from Task 4, `MidiBus` from `crate::midi`.
- Produces:
  - `pub async fn handle_event(core: &mut Core, ui: &mut Controller, midi: &MidiController, output: &mut OutputSender, event: AppEvent)`
  - `pub async fn run_core(core: &mut Core, receiver: AppReceiver, output: OutputSender, ui_writer: UiWriter) -> !`
  - `pub async fn run_midi_dispatch<B: MidiBus>(bus: &mut B, receiver: OutputReceiver) -> !`

- [ ] **Step 1: Write the failing test**

Add to `src/runtime/core_task.rs`:

```rust
#[test]
fn midi_note_reaches_core_output() {
    block_on(async {
        let storage: &'static mut RuntimeStorage = Box::leak(Box::new(RuntimeStorage::new()));
        let mut handles = storage.split();
        let mut core = Core::new(handles.output_sender.clone(), handles.animators);
        let mut ui = Controller::new(
            handles.ui_writer.clone(),
            crate::core::Selection { voice: VoiceId(0), lane: 2 },
        );
        let note = MidiMsg::ChannelVoice {
            channel: Channel::Ch1,
            msg: ChannelVoiceMsg::NoteOn { note: 60, velocity: 100 },
        };

        handle_event(
            &mut core,
            &mut ui,
            &MidiController::new(),
            &mut handles.output_sender,
            AppEvent::Midi(note),
        )
        .await;

        assert!(handles.output_receiver.try_recv().is_ok());
    });
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --manifest-path tests/host/Cargo.toml --target x86_64-unknown-linux-gnu --offline midi_note_reaches_core_output`
Expected: FAIL — `handle_event` does not exist.

- [ ] **Step 3: Extract the bodies**

Move the body of `src/main.rs:397-420` into `handle_event`. Move the loop from `src/main.rs:381-421` into `run_core`; it calls `handle_event` for each event. Move `src/main.rs:350-366` into `run_midi_dispatch`. Keep behavior identical.

- [ ] **Step 4: Rewire the RTIC tasks**

Make `core_task` and `midi_dispatch` one-line wrappers around `crate::runtime::run_core` and `crate::runtime::run_midi_dispatch`.

- [ ] **Step 5: Verify**

Run: `cargo check`
Expected: PASS.

Run: `cargo test --manifest-path tests/host/Cargo.toml --target x86_64-unknown-linux-gnu --offline`
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/runtime src/main.rs
git commit -m "[Refactor] Extract core and MIDI dispatch task bodies"
```

---

### Task 6: Extract Buttons and Animations

**Files:**
- Create: `src/runtime/buttons.rs`
- Create: `src/runtime/animations.rs`
- Modify: `src/runtime/mod.rs`
- Modify: `src/ui/hardware.rs:42-84`
- Modify: `src/ui/buttons.rs`
- Modify: `src/main.rs:505-525`
- Delete: `src/anim/`
- Test: `src/ui/buttons.rs`

**Interfaces:**
- Consumes: `Clock` from Task 2, `ButtonReceiver`, `AppSender`, `AnimationEngine`, `OutputSender`.
- Produces:
  - `pub async fn run_buttons<C: Clock>(receiver: ButtonReceiver, sender: AppSender) -> !`
  - `pub async fn run_animations<C: Clock>(engines: [[AnimationEngine; ANIMATORS_PER_VOICE]; MAX_VOICES], sender: OutputSender)`

- [ ] **Step 1: Write the failing debounce test**

Add to `src/ui/buttons.rs`:

```rust
#[test]
fn held_button_emits_one_press_and_one_release() {
    let mut button = ButtonDebouncer::new();
    assert!(button.update(0, true));
    assert!(!button.update(10, true));
    assert!(!button.update(20, false));
    assert!(!button.take_release(40));
    assert!(button.take_release(50));
    assert!(!button.take_release(60));
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --manifest-path tests/host/Cargo.toml --target x86_64-unknown-linux-gnu --offline held_button_emits_one_press_and_one_release`
Expected: FAIL — `ButtonDebouncer` is private to `ui/mod.rs`.

- [ ] **Step 3: Make the debouncer testable and move the runtimes**

Re-export `ButtonDebouncer` from `src/ui/mod.rs`. Move `run_buttons` from `src/ui/hardware.rs:42-76` into `src/runtime/buttons.rs`, replacing `Systick` with `C`. Move `src/anim/runtime.rs` into `src/runtime/animations.rs` and delete `src/anim/`.

- [ ] **Step 4: Rewire call sites**

`src/ui/hardware.rs` keeps only the GPIO ISR helpers and slider reads. `src/main.rs` calls `crate::runtime::run_buttons::<SystickClock>(...)` and `crate::runtime::run_animations::<SystickClock>(...)`.

- [ ] **Step 5: Verify**

Run: `cargo check`
Expected: PASS.

Run: `cargo test --manifest-path tests/host/Cargo.toml --target x86_64-unknown-linux-gnu --offline`
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/runtime src/ui src/main.rs
git rm -r src/anim
git commit -m "[Refactor] Extract button and animation runtimes"
```

---

### Task 7: Portable OLED Renderer

**Files:**
- Create: `src/ui/display/renderer.rs`
- Modify: `src/ui/display/mod.rs`
- Modify: `src/ui/display/envelope.rs`
- Modify: `src/ui/mod.rs`
- Modify: `tests/host/src/lib.rs`
- Modify: `tests/host/Cargo.toml`
- Test: `tests/host/src/renderer_test.rs`

**Interfaces:**
- Consumes: `DisplaySnapshot`, `UiModel`, `envelope_plot_level`.
- Produces: `pub fn draw_snapshot<D: DrawTarget<Color = BinaryColor>>(display: &mut D, snapshot: &DisplaySnapshot) -> Result<(), D::Error>`

- [ ] **Step 1: Write the failing tests**

Add `embedded-graphics = "0.8"` to `tests/host/Cargo.toml`. Add `#[path = "../../../src/ui/display/renderer.rs"] mod renderer;` and a `#[cfg(test)] mod renderer_test;` to `tests/host/src/lib.rs`. Create `tests/host/src/renderer_test.rs` with:

```rust
use super::renderer::draw_snapshot;

#[test]
fn passthrough_snapshot_draws_a_label() {
    let mut display = TestDisplay::new();
    draw_snapshot(&mut display, &passthrough_snapshot()).unwrap();
    assert!(display.lit_pixels() > 0);
}

#[test]
fn out_of_bounds_drawing_is_ignored() {
    let mut display = TestDisplay::new();
    draw_snapshot(&mut display, &passthrough_snapshot()).unwrap();
    assert_eq!(display.size(), Size::new(128, 64));
}
```

`TestDisplay` is a 128×64 `DrawTarget` defined in that test file. It stores one bit per pixel, counts lit pixels, and ignores points outside 0..128 × 0..64.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test --manifest-path tests/host/Cargo.toml --target x86_64-unknown-linux-gnu --offline passthrough_snapshot_draws_a_label`
Expected: FAIL — `draw_snapshot` does not exist.

- [ ] **Step 3: Extract the drawing body**

Move the drawing code from `src/ui/display/envelope.rs:34-125` into `draw_snapshot`, replacing `oled` with the generic `display`. Keep the exact text, positions, and line sampling. In `src/ui/display/mod.rs`, include `renderer` unconditionally and gate only `envelope`; re-export `draw_snapshot` from `src/ui/mod.rs`.

- [ ] **Step 4: Verify**

Run: `cargo check`
Expected: PASS.

Run: `cargo test --manifest-path tests/host/Cargo.toml --target x86_64-unknown-linux-gnu --offline`
Expected: all tests pass, including the renderer tests.

- [ ] **Step 5: Commit**

```bash
git add src/ui/display src/ui/mod.rs tests/host
git commit -m "[Refactor] Extract portable OLED renderer"
```

---

### Task 8: Move Teensy Code Under `teensy`

**Files:**
- Create: `src/teensy/mod.rs`
- Create: `src/teensy/rtic_app.rs`
- Create: `src/teensy/board.rs`
- Create: `src/teensy/clock.rs`
- Create: `src/teensy/display.rs`
- Create: `src/teensy/hardware.rs`
- Create: `src/teensy/midi.rs`
- Move: `src/usb/` → `src/teensy/usb/`
- Move: `src/diagnostics.rs` → `src/teensy/diagnostics.rs`
- Modify: `src/main.rs`
- Modify: `src/midi/mod.rs`
- Modify: `src/ui/mod.rs`

**Interfaces:**
- Consumes: `runtime`, `ui`, `core`.
- Produces: `teensy::SystickClock`, `teensy::midi::{UartMidiBus, UsbMidiBus, Bus}`, `teensy::display::run_envelope_display`, `teensy::app`.

- [ ] **Step 1: Move the RTIC app**

Move the whole `mod app { ... }` block into `src/teensy/rtic_app.rs` as `#[rtic::app] pub mod app { ... }`. Keep every attribute and declaration unchanged.

- [ ] **Step 2: Move board setup**

Extract the pin, ADC, GPIO, I2C, and logger setup from `app::init` into `src/teensy/board.rs`. `app::init` calls `board::init(cx.device)` and receives a struct of hardware handles.

- [ ] **Step 3: Move the remaining Teensy modules**

Move `src/usb/`, `src/diagnostics.rs`, `src/ui/hardware.rs`, and the `UartMidiBus`/`UsbMidiBus`/`Bus` definitions from `src/midi/mod.rs` into `src/teensy/`. Keep `MidiBus` and `MidiController` in `src/midi/`. Remove the hardware re-exports from `src/ui/mod.rs`.

- [ ] **Step 4: Reduce `main.rs`**

`src/main.rs` keeps the crate attributes, panic handler, fault handler, `extern crate alloc`, and `mod` declarations. All bodies delegate to `teensy`.

- [ ] **Step 5: Verify**

Run: `cargo check`
Expected: PASS.

Run: `cargo test --manifest-path tests/host/Cargo.toml --target x86_64-unknown-linux-gnu --offline`
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/teensy src/midi src/ui
git commit -m "[Refactor] Move Teensy-specific code under teensy module"
```

---

### Task 9: Host Feature and Cargo Layout

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/main.rs`
- Create: `src/host/mod.rs`

**Interfaces:**
- Consumes: `runtime`, `ui`, `core`.
- Produces: `host::run()` and the `host` feature.

- [ ] **Step 1: Add the feature set**

Add:

```toml
[features]
default = ["teensy"]
teensy = []
host = ["dep:tokio", "dep:embedded-graphics-simulator", "dep:critical-section"]

[dependencies]
tokio = { version = "1", features = ["rt", "time"], optional = true }
embedded-graphics-simulator = { version = "0.8", optional = true }
critical-section = { version = "1.2", features = ["std"], optional = true }
```

Add a compile-time guard requiring exactly one platform feature.

- [ ] **Step 2: Make `main.rs` dispatch**

Use `#![cfg_attr(feature = "teensy", no_std)]` and `#![cfg_attr(feature = "teensy", no_main)]`. Gate `mod teensy;` on `feature = "teensy"` and `mod host;` on `feature = "host"`. Gate the panic handler, fault handler, and `extern crate alloc` on `feature = "teensy"`. Add a host `fn main() { host::run() }` gated on `feature = "host"`.

- [ ] **Step 3: Add a stub host entry**

Create `src/host/mod.rs` with `pub fn run() -> !` that builds a current-thread runtime and blocks forever. This exists only to prove the feature wiring.

- [ ] **Step 4: Verify**

Run: `cargo check`
Expected: PASS.

Run: `cargo check --no-default-features --features host --target x86_64-unknown-linux-gnu`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src/main.rs src/host
git commit -m "[Feature] Add host platform feature"
```

---

### Task 10: Host Clock, MIDI Bus, Input, and Display

**Files:**
- Create: `src/host/clock.rs`
- Create: `src/host/midi.rs`
- Create: `src/host/input.rs`
- Create: `src/host/display.rs`
- Modify: `src/host/mod.rs`
- Modify: `tests/host/src/lib.rs`
- Test: `tests/host/src/host_input_test.rs`

**Interfaces:**
- Consumes: `Clock`, `MidiBus`, `AppSender`, `UiEvent`.
- Produces:
  - `pub struct TokioClock;` implementing `Clock`
  - `pub struct HostMidiBus;` implementing `MidiBus`
  - `pub enum HostKey { Button(u8), EncoderCw, EncoderCcw, EncoderClick, SliderUp(u8), SliderDown(u8), Note(u8), UsbToggle }`
  - `pub enum HostEvent { App(AppEvent), Button(ButtonEdge), Encoder(UiEvent) }`
  - `pub struct HostInput { ... }` with `pub fn handle(&mut self, key: HostKey, pressed: bool, now_ms: u32) -> Option<HostEvent>`

- [ ] **Step 1: Write the failing tests**

Add `#[path = "../../../src/host/input.rs"] mod host_input;` and `#[cfg(test)] mod host_input_test;` to `tests/host/src/lib.rs`. Create `tests/host/src/host_input_test.rs` with:

```rust
use super::host_input::{HostEvent, HostInput, HostKey};

#[test]
fn note_key_emits_note_on_and_off() {
    let mut input = HostInput::new();
    assert!(matches!(input.handle(HostKey::Note(60), true, 0), Some(HostEvent::App(_))));
    assert!(matches!(input.handle(HostKey::Note(60), false, 1), Some(HostEvent::App(_))));
}

#[test]
fn button_key_emits_a_debounced_edge() {
    let mut input = HostInput::new();
    assert!(matches!(
        input.handle(HostKey::Button(0), true, 0),
        Some(HostEvent::Button(ButtonEdge { index: 0, pressed: true, at_ms: 0 }))
    ));
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test --manifest-path tests/host/Cargo.toml --target x86_64-unknown-linux-gnu --offline host_input`
Expected: FAIL — `HostInput` does not exist.

- [ ] **Step 3: Implement the host pieces**

`TokioClock` uses `std::time::Instant` and `tokio::time::sleep`. `HostMidiBus::poll` is a no-op; `send` logs the message. `HostInput` maps button and encoder-click keys to `HostEvent::Button`, encoder rotation to `HostEvent::Encoder`, and notes, sliders, and USB status to `HostEvent::App`. Slider keys step a stored `[u16; 4]` by 4096.

- [ ] **Step 4: Implement the display and task graph**

Create a 128×64 `SimulatorDisplay<BinaryColor>` and a `Window` with an OLED theme and 4× scale. In `host::run()`:

- Build `runtime::Builder::new_current_thread().enable_time().build()`.
- Create a `StaticCell<RuntimeStorage>` and split it.
- Create a `LocalSet`.
- Spawn local tasks for `run_core`, `run_animations::<TokioClock>`, `run_buttons::<TokioClock>`, `run_midi_dispatch`, and the display loop.
- Dispatch `HostEvent::App` to `app_sender`, `HostEvent::Button` to `button_sender`, and `HostEvent::Encoder` to `app_sender` as `AppEvent::Ui`.

- [ ] **Step 5: Verify**

Run: `cargo test --manifest-path tests/host/Cargo.toml --target x86_64-unknown-linux-gnu --offline`
Expected: PASS.

Run: `cargo run --no-default-features --features host --target x86_64-unknown-linux-gnu`
Expected: an OLED window opens and responds to keys.

- [ ] **Step 6: Commit**

```bash
git add src/host tests/host
git commit -m "[Feature] Add host OLED simulator"
```

---

### Task 11: Verification and Documentation

**Files:**
- Modify: `AGENTS.md`
- Modify: `CLAUDE.md`
- Modify: `tests/host/src/lib.rs`
- Modify: `tests/host/src/boundary_test.rs`

- [ ] **Step 1: Run the full verification**

```bash
cargo check
cargo test --manifest-path tests/host/Cargo.toml --target x86_64-unknown-linux-gnu --offline
cargo check --no-default-features --features host --target x86_64-unknown-linux-gnu
```

Expected: all pass.

- [ ] **Step 2: Add a boundary test**

Create `tests/host/src/boundary_test.rs`, include it from `tests/host/src/lib.rs` with `#[cfg(test)] mod boundary_test;`, and have it read `src/core/**` and `src/runtime/**` and assert none contain `teensy4_bsp`, `teensy_usbhost`, `cortex_m`, `rtic::app`, `rtic_monotonics`, or `tokio::`.

- [ ] **Step 3: Document the split**

Add to `AGENTS.md`:

- The `core` / `runtime` / `teensy` / `host` boundary.
- The two build commands.
- The rule that the host does not model RTIC priorities.

- [ ] **Step 4: Commit**

```bash
git add AGENTS.md CLAUDE.md tests/host
git commit -m "[Docs] Document platform split and host simulator"
```
