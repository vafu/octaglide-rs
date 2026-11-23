# Runtime Extraction and Host Simulator Design

**Date:** 2026-11-23  
**Status:** Proposed  
**Scope:** Refactor the firmware into portable core/runtime layers, isolate Teensy code, and add a host simulator.

## Intent

The project currently mixes portable domain logic with Teensy-specific RTIC wiring. That makes it difficult to exercise the application without hardware and harder to change board support later.

The goal is to make the application runnable in two places from the same code:

- On the Teensy 4.1, under RTIC.
- On the development machine, under a single-threaded Tokio runtime with a simulated OLED and keyboard input.

The host simulator is the primary local development target. The Teensy remains the authority for hardware behavior.

## Goals

- Extract `core` so it has no dependency on RTIC, Teensy, Tokio, or a specific clock.
- Extract shared async task bodies into a `runtime` module.
- Host all Teensy-specific code under `src/teensy/`.
- Add a host application under `src/host/`.
- Reuse the same bounded channels, task graph, and OLED renderer on both platforms.
- Keep `rtic-sync` as the channel implementation.
- Keep the firmware build working throughout the refactor.

## Non-Goals

- QEMU, Renode, or any instruction-level emulation.
- Emulating i.MX peripherals, USB host hardware, or SH1106 I2C timing.
- Reproducing RTIC preemption or task priorities on the host.
- Replacing `rtic-sync` with `tokio::sync`.
- Changing MIDI routing, envelope behavior, or UI semantics.
- Supporting more than one host platform in this phase.

## Anti-Goals

- Adding a `#[cfg(target_os = "none")]` maze as the primary abstraction mechanism.
- Duplicating task logic between firmware and host.
- Introducing a new async runtime into the firmware.
- Making `core` aware of display or hardware details.

## Constraints

- `rtic-sync` is executor-agnostic and is already used successfully in host tests.
- `rtic-sync::Sender` is `Send` but not `Sync`; senders must be cloned rather than shared by reference across Tokio tasks.
- `make_channel!` creates function-local statics and panics if invoked twice.
- `critical-section` must resolve to the Cortex-M implementation on Teensy and the `std` implementation on the host.
- `#[rtic::app]` generates a `#[no_mangle] extern "C" fn main`, so the app may live in a nested module without losing the entry symbol.
- The crate needs `#![no_std]`, `#![no_main]`, a panic handler, and a global allocator only when the Teensy feature is selected.
- `embedded-graphics-simulator::Window` is not `Send`; the SDL window must stay on one thread.
- The existing host test harness uses path includes rather than a library target.
- The firmware currently targets `thumbv7em-none-eabihf` via `.cargo/config.toml`.

## Approaches Considered

### 1. Cooperative priority executor

Build a small single-threaded executor that polls futures in RTIC priority order, with host input injected before each cycle.

**Pros:** Closest to RTIC ordering; deterministic.  
**Cons:** More machinery, custom executor semantics, and still cooperative rather than preemptive.

### 2. Single-threaded Tokio with one task per component

Use `tokio::runtime::Builder::new_current_thread()` with a `LocalSet`, and spawn one local task per logical component. Reuse the same shared task bodies and channels.

**Pros:** Simplest; idiomatic Tokio; minimal new code; single-threaded like the target; `LocalSet` accepts the non-`Send` SDL window and non-`Send` futures.  
**Cons:** No priority ordering. Tokio explicitly makes no scheduling-order guarantee.

### 3. Priority-tier event loop

Use a single loop that drains priority-4 input, then polls tier-3, tier-2, and tier-1 handlers.

**Pros:** Models event priority without a custom executor.  
**Cons:** Still not RTIC priority semantics, and complicates the host entry point.

### Decision

Approach 2 is selected.

The host is a development aid, not a timing validator. The simplest implementation that reuses the real task graph is preferable. If starvation or ordering problems appear during use, the fallback is Approach 1 or 3, applied only to the host entry point.

## Architecture

```text
src/
  main.rs                    # platform dispatch and crate-root handlers
  core/                      # domain logic; no platform dependencies
  runtime/                   # executor-agnostic async task bodies
    mod.rs
    storage.rs
    core_task.rs
    midi_dispatch.rs
    buttons.rs
    animations.rs
  ui/
    controller.rs
    snapshot.rs
    buttons.rs
    encoder.rs
    envelope_plot.rs
    display/
      mod.rs
      renderer.rs
  teensy/
    mod.rs
    rtic_app.rs
    board.rs
    clock.rs
    display.rs
    hardware.rs
    midi.rs
    usb/
    diagnostics.rs
    panic.rs
  host/
    mod.rs
    clock.rs
    midi.rs
    display.rs
    input.rs
```

Dependency direction:

```text
core <- runtime <- teensy
core <- runtime <- host
ui   <- runtime
```

No module may depend back toward `teensy` or `host`.

## Core Extraction

`core` becomes platform-neutral.

- Add `core::Animator = Sender<'static, Cmd, 1>`.
- Replace all `crate::app::Animator` references in `core`.
- Add a `Clock` trait:

```rust
pub trait Clock {
    fn now_ms() -> u32;
    async fn delay_ms(ms: u32);
}
```

- Keep `AnimationEngine` non-generic; make only `tick<C: Clock>(&mut self)` generic.
- Store elapsed time as `u32` milliseconds rather than a `rtic_monotonics` instant.
- Remove `rtic_monotonics` from `core`.
- Make the animation runtime generic over `Clock`: `run_animations<C: Clock>`.
- Remove the target gate from `normalize_adc_10bit`.

## Runtime Extraction

`runtime` owns the shared task bodies and channel storage.

```rust
pub struct RuntimeStorage {
    pub output: Channel<OutputMessage, 32>,
    pub app: Channel<AppEvent, 16>,
    pub buttons: Channel<ButtonEdge, 16>,
    pub ui: Signal<DisplaySnapshot>,
    pub animators: [[Channel<Cmd, 1>; 3]; MAX_VOICES],
}
```

`RuntimeStorage::split()` returns the senders, receivers, and animation engines. It does not need a clock type because `AnimationEngine` stays non-generic.

Shared task bodies:

- `run_core`
- `handle_event`
- `run_midi_dispatch<B: MidiBus>`
- `run_buttons<C: Clock>`
- `run_animations<C: Clock>`

The bodies are moved from `main.rs`, `ui/hardware.rs`, and `anim/runtime.rs` with behavior unchanged.

## Teensy Module

`teensy` owns everything board-specific.

- The RTIC app and task declarations.
- Board resource construction and pin configuration.
- GPIO interrupt helpers.
- ADC slider reads.
- UART and USB MIDI buses.
- USB host runtime and C callbacks.
- SH1106 display driver and flush loop.
- Diagnostics and panic handling.
- `SystickClock`.

`main.rs` keeps only:

- Platform module selection.
- Teensy-only crate attributes: `#![no_std]`, `#![no_main]`, `extern crate alloc`.
- Teensy-only `#[panic_handler]` and `#[cortex_m_rt::exception]` handlers, delegating into `teensy`.

The RTIC app lives in `src/teensy/rtic_app.rs` as `#[rtic::app] mod app`. The macro emits its entry point with `#[no_mangle]`, so module nesting does not hide it.

## Host Application

The host uses a current-thread Tokio runtime with only the time driver enabled, plus a `LocalSet`, so the SDL window and non-`Send` futures can be driven on one thread.

Components:

- `TokioClock`: `std::time::Instant` and `tokio::time::sleep`.
- `HostMidiBus`: implements `MidiBus`; logs outgoing MIDI.
- `HostInput`: maps keyboard events to both `AppEvent::Ui` and `AppEvent::Midi`.
- `HostDisplay`: 128×64 `SimulatorDisplay<BinaryColor>` and an SDL window.

The host spawns the same logical components as the firmware:

- Core task.
- MIDI dispatch.
- Animations.
- Buttons.
- Display.
- Input.

Host scheduling is deliberately not priority-aware. This is documented as a known limitation.

### Host Input Scope

The simulator covers the full front panel and note input, because a simulator that cannot play notes cannot show envelope or glide behavior.

- Buttons: four keys, press and release, emitted as `ButtonEdge` so `run_buttons` performs the real debounce.
- Encoder: two rotation keys emitting `UiEvent::Encoder` directly; the click key emits `ButtonEdge`.
- Sliders: four keys or key pairs emitting `UiEvent::Slider` with 16-bit values.
- Notes: a QWERTY row emitting `AppEvent::Midi` note-on/note-off, with velocity from a modifier or fixed value.
- USB status: a toggle key emitting `UiEvent::UsbConnection`.

`HostMidiBus` remains output-only: `poll()` is a no-op and outgoing messages are logged. Inbound notes enter through `AppSender` as `AppEvent::Midi`, which is the same path UART and USB input use on the Teensy.

## Runtime Ownership

`runtime::run_core` owns the same responsibilities as the current `core_task`:

- The `ui::Controller` and its selection state.
- `MidiController` translation for incoming MIDI.
- Dispatch of UI events into `Core`.
- Publication of `DisplaySnapshot` after each handled UI event, matching current behavior.

The host and firmware therefore share UI behavior rather than reimplementing it. `RuntimeStorage::split` returns the UI writer/reader and all channels needed by `run_core`.

`run_core` loops on the app receiver and delegates each event to `handle_event`, which is separately testable:

```rust
pub async fn handle_event(
    core: &mut Core,
    ui: &mut Controller,
    midi: &MidiController,
    output: &mut OutputSender,
    event: AppEvent,
)
```

## Display Refactor

Split the current display module:

- `ui/display/renderer.rs` contains `draw_snapshot<D: DrawTarget<Color = BinaryColor>>`.
- `teensy/display.rs` handles SH1106 initialization and flushing.
- `host/display.rs` handles SDL presentation.

The renderer is shared and must not import `teensy4_bsp`, `sh1106`, or `rtic_monotonics`. Every model must draw entirely within the 128×64 OLED area.

## Cargo Features

```toml
[features]
default = ["teensy"]
teensy = []
host = ["dep:tokio", "dep:embedded-graphics-simulator"]
```

The build must fail with a clear `compile_error!` when no platform is selected.

Host commands use:

```text
cargo run --no-default-features --features host \
  --target x86_64-unknown-linux-gnu
```

## Verification

| Change | Command |
|---|---|
| Core extraction | `cargo check` |
| Host semantics | `cargo test --manifest-path tests/host/Cargo.toml --target x86_64-unknown-linux-gnu --offline` |
| Teensy build | `cargo check` |
| Host simulator | `cargo run --no-default-features --features host --target x86_64-unknown-linux-gnu` |

## Risks

- **RTIC macro placement.** If the app must remain at crate root, keep a thin wrapper and move all logic into `teensy`.
- **Critical-section implementations.** Only one implementation may be active per build.
- **Feature unification.** Host commands must disable default features.
- **Channel storage reuse.** Use `StaticCell` rather than repeated `make_channel!` calls.
- **SDL backend.** If the window fails under Wayland, retry with `SDL_VIDEODRIVER=x11`.
- **Scheduling assumptions.** The host does not reproduce RTIC priorities. Timing conclusions must come from hardware.

## Builder Decisions

- Use `static_cell` for runtime storage.
- Keep the host on a current-thread runtime.
- Keep `rtic-sync` rather than switching to Tokio channels.
- Keep the existing host test crate rather than converting the firmware into a library in this phase.
- Defer any priority-emulation work until a concrete problem appears.
