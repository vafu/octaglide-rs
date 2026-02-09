# OctaglideRS - MIDI Stream Processor

## Project Overview

OctaglideRS is an embedded Rust MIDI processor that runs on a Teensy 4.1 microcontroller. It sits between a MIDI sequencer (e.g., Octatrack) and a synthesizer, processing and modifying the MIDI stream in real-time.

**Current Status**: Early development - core glide feature implemented, USB Host MIDI support in progress.

**Name Origin**: "Octatrack" (primary controller) + "Glide" (first feature) = OctaglideRS

**Task Management**: All project tasks and planning are managed via Backlog.md (see BACKLOG.MD MCP section at the bottom of this file)

## Hardware Setup

```
[MIDI Sequencer] --MIDI--> [Teensy 4.1 / OctaglideRS] --MIDI--> [Synthesizer]
    (Octatrack)              (protoboard)
```

- **Platform**: Teensy 4.1 (ARM Cortex-M7)
- **MIDI**: LPUART6 @ 31250 baud (standard MIDI) + USB Host MIDI
- **USB Host**: Using built-in USB Host pins with breakout to USB-A connector
- **Debug**: USB logging via imxrt-log (USB device port)
- **Current State**: Prototype on protoboard, all hardware subject to change

## Architecture

### High-Level Data Flow

```
MIDI Input (UART RX)
    ↓
MidiBus (interrupt-driven RX/TX)
    ↓
Core::process_midi()
    ↓
Transformers (serial chain) ← synchronous, in-place modifications
    ↓
Consumers (parallel) ← async, can spawn animations
    ↓
Animator (optional) ← generates timed MIDI sequences
    ↓
Dispatcher
    ↓
MidiBus (UART TX)
    ↓
MIDI Output
```

### Key Architectural Concepts

1. **Transformers** (`src/core/transformers.rs`)
   - Synchronous, in-place MIDI message modification
   - Execute serially in defined order
   - Can filter messages (return `None`) or modify them
   - Examples: octave shifting, velocity scaling, channel remapping
   - **Why serial**: Order matters (e.g., octave shift before glide)

2. **Consumers** (`src/core/consumers/`)
   - Final processing stage that can spawn async operations
   - Can generate multiple output messages
   - Can trigger animations via the Animator
   - Examples: glide, arpeggiators, chord generators
   - **Why async**: Need to generate timed sequences of MIDI messages

3. **Animator** (`src/anim/animator.rs`)
   - Manages time-based MIDI message generation
   - Runs at ~5ms tick intervals (MSG_INTERVAL_MS)
   - Supports progress-based modulation (0.0 to 1.0)
   - States: Idle, Animating
   - Commands: Start(Modulator), Stop, Duration(ms)

4. **Modulators** (`src/anim/modulators.rs`)
   - Define animation behaviors (e.g., glide trajectory)
   - Implement `Modulation` trait: `animate(progress, depth, offset)` and `reset()`
   - Generate MIDI messages at each animation tick

## Project Structure

```
octaglide-rs/
├── src/
│   ├── main.rs              # RTIC app, hardware init, task scheduling
│   ├── midi.rs              # MidiBus: interrupt-driven UART MIDI I/O
│   ├── midi_fmt.rs          # Display formatting helpers for MIDI messages
│   ├── usb_callbacks.rs     # C++ callback implementations (rust_log_info, rust_micros)
│   ├── core/
│   │   ├── mod.rs          # Core: main processing pipeline
│   │   ├── transformers.rs # Synchronous MIDI transformers
│   │   └── consumers/
│   │       ├── mod.rs      # Consumer trait
│   │       └── glider.rs   # Glide implementation
│   └── anim/
│       ├── mod.rs
│       ├── animator.rs     # Animator
│       └── modulators.rs   # Modulator trait + implementations (Glide)
│
└── teensy-usbhost/          # USB Host library wrapper (separate crate)
    ├── src/
    │   └── lib.rs          # Rust bindings to C++ USB host library
    ├── sys/
    │   └── shim.cpp        # C++ USB host integration
    └── build.rs            # C++ compilation via cc crate
```

## Current Features

### 1. Glide (Portamento) Effect
**Location**: `src/core/consumers/glider.rs` + `src/anim/modulators.rs`

**How it works**:
- Tracks up to 8 held notes (monophonic glide behavior)
- When a new note is played while another is held, triggers a glide animation
- Uses pitchbend to smoothly transition between notes
- Intelligently switches active notes during glide to stay within synth's pitchbend range (±2 semitones assumed)

**Algorithm** (`Glide::animate`):
1. Calculate interpolated note position based on animation progress
2. Determine which physical note should be playing (from, to, or intermediate)
3. If active note changes, send NoteOn for new note + NoteOff for old note
4. Send PitchBend to achieve precise pitch

**Known Issues**:
- Check Backlog.md for current bugs and issues

### 2. Octave Shifter
**Location**: `src/core/transformers.rs`

**How it works**:
- Listens for CC20 (test CC, will be configurable later)
- Maps CC value (0-127) to octave shifts: (value - 64) / 16 * 12 semitones
- Applies offset to all NoteOn/NoteOff messages
- Clamps result to valid MIDI range (0-127)

### 3. Hardware Reset Button
**Location**: `src/main.rs` (reset_button_isr)

**How it works**:
- P14 (GPIO1_IO18) configured as input with 100kΩ pulldown
- Wire momentary button from P14 to 3.3V
- Interrupt-driven on rising edge (button press)
- Immediately resets MCU via `cortex_m::peripheral::SCB::sys_reset()`

**Implementation details**:
- Uses GPIO1_COMBINED_16_31 interrupt (P14 is GPIO1 pin 18, which is in the 16-31 range)
- Priority 2 (same as other ISRs)
- No debouncing needed - interrupt fires once, MCU resets immediately
- Important: P14 uses GPIO1_COMBINED_16_31, not GPIO1_COMBINED_0_15!
- RTIC automatically enables the interrupt via `#[task(binds = ...)]` - no manual NVIC setup needed

**GPIO Interrupt Mapping Reference** (for future encoder work):
- GPIO1 pins 0-15 → `GPIO1_COMBINED_0_15`
- GPIO1 pins 16-31 → `GPIO1_COMBINED_16_31`
- GPIO2 pins 0-15 → `GPIO2_COMBINED_0_15`
- GPIO2 pins 16-31 → `GPIO2_COMBINED_16_31`
- (etc for GPIO3, GPIO4...)

To enable a GPIO interrupt:
```rust
// Configure pin and set interrupt trigger
gpio_port.set_interrupt(&input_pin, Some(gpio::Trigger::RisingEdge));

// RTIC handles NVIC enabling automatically via #[task(binds = GPIO*_COMBINED_*)]
```

## RTIC (Real-Time Interrupt-driven Concurrency)

The project uses RTIC 2.x for task scheduling and resource management.

### Key Tasks

| Task | Priority | Type | Purpose |
|------|----------|------|---------|
| `midi_handler` | 2 | ISR | UART interrupt handler (RX/TX) |
| `log_over_usb` | 2 | ISR | USB logging |
| `usb_host_isr` | 2 | ISR | USB host controller interrupt |
| `midi_dispatch` | 2 | async | Send MIDI messages from queue |
| `animate` | 2 | async | Animator tick loop |
| `usb_host_init` | 1 | async | USB host initialization (delayed) |
| `usb_host_test` | 2 | async | USB MIDI device polling and testing |
| `process_input` | 1 | async | Core MIDI processing pipeline |
| `blink_led` | 1 | async | LED feedback (debugging) |

### Shared Resources
- `MidiBus`: Shared between interrupt handlers and async tasks (via `lock()`)

### Channels
- `MidiMsg` channel (capacity: 16): For outgoing MIDI messages
- `Cmd` channel (capacity: 1): For animator commands

### USB Host Integration

The project uses the `teensy-usbhost` crate for USB Host MIDI support. This is a separate crate that wraps the Teensy C++ USB Host library.

**Key Implementation Details:**
- **C++ Callbacks**: The main application provides callback functions (`rust_log_info`, `rust_micros`) in `src/usb_callbacks.rs`
- **Logging Callback**: `rust_log_info()` must be in the main app where the logger is initialized
- **Timer Callback**: `rust_micros()` currently in main app (TODO: should be in teensy-usbhost once callback indirection issue is resolved)

**USB Host `task()` Function (CRITICAL):**
The `usbhost::task()` function advances the USB state machine. **It only needs to be called:**
1. **During enumeration** - in a loop until device connects (state machine advancement)
2. **On USB interrupt** - when the USB_OTG2 interrupt fires (transfer init)

**DO NOT call `task()` periodically after enumeration!** The state machine is event-driven:
- Enumeration requires polling to advance the state machine
- Once connected, only USB interrupts (transfer init) need to trigger `task()`
- Periodic calls waste CPU and are unnecessary

**Current Implementation:**
- `usb_host_maintenance` task: Loops calling `task()` until device connects, then exits
- `usb_host_isr` (USB_OTG2): Calls `task()` on USB transfer init interrupts
- This provides optimal CPU usage while ensuring reliable operation

**Initialization Order:**
1. `init()` task sets up logging via `imxrt-log` (USB device mode)
2. `usb_host_init` task initializes USB host C++ code
3. `usb_host_maintenance` task loops calling `task()` until device enumerates
4. Once connected, only `usb_host_isr` calls `task()` (interrupt-driven)

## Important Constants

```rust
HEAP_SIZE: 1024 bytes           // Embedded allocator
MIDI_BAUD: 31250                // Standard MIDI baud rate
MIDI_BUF_SIZE: 32               // RX/TX buffer size
MSG_INTERVAL_MS: 5              // Animation tick interval
SYNTH_BEND_RANGE_SEMITONES: 2.0 // Assumed synth pitchbend range
PITCHBEND_CENTER: 8192          // MIDI pitchbend center value
MAX_HELD_NOTES: 8               // Glider note tracking
```

## Future Roadmap

Planned features and known bugs are tracked in Backlog.md. See the BACKLOG.MD MCP section at the bottom of this file for workflow instructions.

## Development Notes

### Building & Flashing
```bash
# Build for Teensy 4.1 (thumbv7em-none-eabihf target)
cargo build --release

# Convert ELF to Intel HEX format for Teensy bootloader
cargo objcopy -- -O ihex -R .init_array target/thumbv7em-none-eabihf/release/octaglide-rs.hex

# Flash to Teensy (tooling TBD - check project scripts)
```

**Important: The `-R .init_array` flag is required**

The `-R .init_array` flag removes the `.init_array` ELF section during conversion to Intel HEX. **Without this flag, objcopy will fail.**

**Why it fails:**
- The `.init_array` section contains C++ global constructor pointers
- Teensy's linker script doesn't properly place this section in the memory map
- When objcopy tries to convert to HEX, it encounters an invalid/out-of-bounds address
- This causes the conversion to fail with address errors

**Why we don't need it:**
- RTIC handles all initialization via the `#[init]` task
- No C runtime to process global constructors
- The section is generated by LLVM even in `#![no_std]` mode but is unused

This is a known pattern with Teensy + Rust + cargo-objcopy, working around impedance mismatch between Rust's LLVM backend and Teensy's bare-metal environment.

### Testing Setup
- MIDI Controller: Octatrack (or any MIDI controller)
- Device: Teensy 4.0 on protoboard
- Synth: Any MIDI-compatible synthesizer
- All components subject to change

### Debugging
- USB logging available via `log::info!()`, `log::error!()`, etc.
- Messages prefixed with `<<<` (incoming) and `>>>` (outgoing) in `core/mod.rs` and `midi.rs`
- LED blink on specific events (currently used for debugging)

### Code Style Notes
- `#![no_std]` - no standard library (embedded)
- Uses `alloc` with custom heap allocator (`embedded-alloc`)
- Heavy use of `heapless` for fixed-size collections (no dynamic allocation in hot paths)
- RTIC tasks use `async`/`await` for cooperative multitasking

### Error Handling Policy
**Current (Development)**: Using `unwrap()` on channel sends to catch buffer overflows early and panic immediately.

**TODO BEFORE RELEASE**: Review all error handling. For production embedded real-time system:
- Channel send errors should likely log and continue (not panic)
- Consider what happens when channels are full (backpressure strategy)
- Document recovery behavior for each error case
- Test error scenarios thoroughly

## Architecture Extension Ideas

Current architecture is solid but open to suggestions. Potential considerations:

1. **Transformer/Consumer separation**: Works well. Keep it.
   - Transformers = synchronous, chainable, order-dependent
   - Consumers = async, parallel, can spawn animations

2. **Possible additions**:
   - **Filters**: Pre-transformer stage for routing/filtering by channel, type, etc.
   - **Post-processors**: After consumers, before output (e.g., velocity humanization)
   - **Presets**: Save/load transformer + consumer configurations

3. **Configuration system**:
   - Store in EEPROM/Flash (Teensy supports this)
   - MIDI SysEx for remote configuration
   - UI integration (when implemented)

## When Helping with This Project

### DO:
- Follow the `no_std` embedded patterns
- Use `heapless` collections where possible
- Avoid heap allocations wherever possible. Remember the heap limit is 1KB.
- Respect RTIC task priorities and shared resource locking
- Log extensively for debugging (USB logging is available)
- Consider real-time constraints (MIDI timing is critical)
- Consult Backlog.md for current tasks and project planning

### DON'T:
- Use `std` library features
- Perform blocking operations in high-priority tasks
- Allocate large amounts of memory
- Ignore MIDI timing requirements (~3ms jitter is noticeable)
- Assume any specific synth features (pitchbend range, CC mappings, etc.) without configuration

### Common Pitfalls:
- **Forgetting `unsafe` blocks**: Static mutable refs need `#[allow(static_mut_refs)]`
- **UART buffer overflows**: MIDI is fast, buffers are small (32 bytes)
- **Animation timing**: 5ms ticks are fast; keep `animate()` implementations lean
- **Pitchbend calculations**: Easy to make off-by-one errors with 14-bit bend values

## File Modification Guide

### Adding a new Transformer:
1. Implement `MidiTransformer` trait in `src/core/transformers.rs`
2. Add to transformer chain in `Core::new()` in `src/core/mod.rs`
3. Order matters! Earlier transformers run first.

### Adding a new Consumer:
1. Create new module in `src/core/consumers/`
2. Implement `Consumer` trait
3. Add to consumer list in `Core::new()` in `src/core/mod.rs`
4. If it needs animation, send `Output::Animate(Cmd)` messages

### Adding a new Modulator:
1. Add variant to `Modulator` enum in `src/anim/modulators.rs`
2. Implement struct + `Modulation` trait
3. Use from Consumer via `Output::Animate(Cmd::Start(Modulator::YourModulator(...)))`

### Modifying MIDI I/O:
- `src/midi.rs` - be very careful, timing-critical code
- Test thoroughly with real hardware

## Dependencies

**Main Crate:**
- `rtic`: Real-time framework (v2, thumbv7 backend)
- `teensy4-bsp`: Board support package for Teensy 4.x
- `teensy-usbhost`: USB Host MIDI support (local crate)
- `midi-msg`: MIDI message parsing/serialization (no_std compatible)
- `heapless`: Fixed-capacity collections (Vec, Deque, etc.)
- `embedded-alloc`: Heap allocator for embedded systems
- `enum_dispatch`: Zero-cost enum dispatch for traits
- `imxrt-log`: USB logging support

**teensy-usbhost Crate:**
- Wraps Teensy C++ USB Host library
- Built using `cc` crate for C++ compilation
- Provides Rust-safe bindings to USB MIDI functionality

## Questions to Ask User Before Major Changes

1. **Performance targets**: What's the acceptable latency? (Currently ~5-10ms)
2. **Memory constraints**: 1KB heap is very tight - can we increase if needed?
3. **MIDI features**: Which MIDI messages should be supported? (Currently: Note, CC, PitchBend)
4. **Configuration**: How should users configure the device? (MIDI? UI? Compile-time?)
5. **Synth compatibility**: Any specific synths to test against?

## Git flow

- keep commit messages short, just a few words to summarize the change.
- for commit messages use tags in square brackets to reflect status of the change, like `[WIP]`, `[Feature]`, etc.
- don't include "Generated with Claude code", or any extra info. whole commit message should be just a few words. 

---

**This document should be updated as the project evolves, especially when:**
- New features are added
- Architecture changes
- Configuration system is implemented
- UI is added
- Hardware changes


<!-- BACKLOG.MD MCP GUIDELINES START -->

<CRITICAL_INSTRUCTION>

## BACKLOG WORKFLOW INSTRUCTIONS

This project uses Backlog.md MCP for all task and project management activities.

**CRITICAL GUIDANCE**

- If your client supports MCP resources, read `backlog://workflow/overview` to understand when and how to use Backlog for this project.
- If your client only supports tools or the above request fails, call `backlog.get_workflow_overview()` tool to load the tool-oriented overview (it lists the matching guide tools).

- **First time working here?** Read the overview resource IMMEDIATELY to learn the workflow
- **Already familiar?** You should have the overview cached ("## Backlog.md Overview (MCP)")
- **When to read it**: BEFORE creating tasks, or when you're unsure whether to track work

These guides cover:
- Decision framework for when to create tasks
- Search-first workflow to avoid duplicates
- Links to detailed guides for task creation, execution, and completion
- MCP tools reference

You MUST read the overview resource to understand the complete workflow. The information is NOT summarized here.

</CRITICAL_INSTRUCTION>

<!-- BACKLOG.MD MCP GUIDELINES END -->


