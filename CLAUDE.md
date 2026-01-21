# OctaglideRS - MIDI Stream Processor

## Project Overview

OctaglideRS is an embedded Rust MIDI processor that runs on a Teensy 4.1 microcontroller. It sits between a MIDI sequencer (e.g., Octatrack) and a synthesizer, processing and modifying the MIDI stream in real-time.

**Current Status**: Early development - first feature (Glide) is implemented with known bugs to fix. Currently debugging USB Host MIDI enumeration.

**Name Origin**: "Octatrack" (primary controller) + "Glide" (first feature) = OctaglideRS

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
src/
├── main.rs              # RTIC app, hardware init, task scheduling
├── midi.rs              # MidiBus: interrupt-driven UART MIDI I/O
├── core/
│   ├── mod.rs          # Core: main processing pipeline
│   ├── transformers.rs # Synchronous MIDI transformers
│   └── consumers/
│       ├── mod.rs      # Consumer trait
│       └── glider.rs   # Glide implementation (WIP, has bugs)
└── anim/
    ├── mod.rs
    ├── animator.rs     # Animator
    └── modulators.rs   # Modulator trait + implementations (Glide)
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
- There's a bug in the current implementation (acknowledged by user, to be fixed later)

**Future Plans**:
- Implement slide-back: when releasing the current note, glide back to previous held note
- Add depth control for glide intensity

### 2. Octave Shifter
**Location**: `src/core/transformers.rs`

**How it works**:
- Listens for CC20 (test CC, will be configurable later)
- Maps CC value (0-127) to octave shifts: (value - 64) / 16 * 12 semitones
- Applies offset to all NoteOn/NoteOff messages
- Clamps result to valid MIDI range (0-127)

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

### CRITICAL: USB Host Initialization Order

**The USB host C++ code MUST be initialized AFTER the USB logging system is ready.**

The initialization sequence is:
1. `init()` task sets up logging via `imxrt-log` (USB device mode)
2. `usb_host_init` task waits **3 seconds** for logging to be fully operational
3. `cpp_init()` is called, which:
   - Creates the MIDI device driver using placement new
   - Calls `myusb->begin()` to start USB host mode
4. `usb_host_test` task starts polling the USB host state machine

**Why the delay is essential:**
- C++ code uses `rust_log_info()` to send logs back to Rust
- If `cpp_init()` runs before the USB logging poller is ready, logs are lost
- The 3-second delay ensures the USB device logging is fully initialized
- Without proper logging, debugging USB host issues is impossible

**DO NOT:**
- Call `cpp_init()` from the `init()` function directly
- Remove or reduce the 3-second delay without testing logging works
- Try to "optimize" by moving USB host init earlier in the boot sequence

### USB Host Power (VCC) Pin Issue - ACTIVE DEBUGGING

**Status**: UNRESOLVED - USB host VCC pin (GPIO_EMC_40 / GPIO8 bit 26) outputs 0V on Rust firmware but works correctly on C++ Arduino firmware with identical hardware.

**Hardware Context**:
- Teensy 4.1 USB host power is controlled by GPIO_EMC_40 (GPIO8_IO26)
- This pin must output 5V to power connected USB devices
- Pin is not exposed as standard Arduino pin - internal to USB host circuitry

**Root Cause Analysis** (2026-01-19):

1. **teensy4-bsp Clock Gate Whitelist Issue**:
   - BSP's `prepare_clocks_and_power()` only enables specific clock gates via `CLOCK_GATES` array
   - `CLOCK_GATES` includes GPIO1-4 but **NOT** GPIO5-9
   - Result: GPIO8 peripheral clock (CCM_CCGR3 bits 16-17) left disabled by BSP
   - **Fix Applied**: Re-enable CCGR3 after BSP init in `main.rs:209`
   - **Status**: VERIFIED WORKING - GPIO8 clock now enabled (reads 0x3)

2. **GPIO8 Direction Register Issue** (CURRENT PROBLEM):
   - BSP's `into_pads()` configures IOMUXC mux to ALT5 (GPIO mode) for GPIO_EMC_40
   - **BUT** GPIO8 direction register (GPIO8_GDIR) left at reset default (INPUT)
   - INPUT mode = high-impedance, cannot drive voltage
   - **Symptoms** (from logs):
     ```
     Pre-init: IOMUXC MUX = ALT5 (GPIO mode) - OK
     Pre-init: ERROR - GPIO8 direction = INPUT!
     Pre-init: GPIO8 data register = LOW
     ```
   - **Attempted Fix #1**: Added GPIO8 OUTPUT configuration in `main.rs:212-226`
   - **Status**: FAILED - Used wrong GPIO8 base address (0x401C4000 instead of 0x42008000)
   - **Fix #2**: Corrected GPIO8_BASE to 0x42008000 (from imxrt.h: IMXRT_GPIO8_ADDRESS)
   - **Status**: TESTING - Rebuild complete, awaiting flash/verification

**Comparison with Working C++ Firmware**:
- Arduino framework initializes ALL GPIO peripherals during startup
- By the time `myusb.begin()` runs, GPIO8 is already functional
- Rust BSP selective initialization leaves GPIO8 unconfigured

**Investigation Path**:
```
✅ CCM_CCGR3 (GPIO8 clock)     - ENABLED (0x3)
✅ IOMUXC MUX (GPIO mode)      - Configured (ALT5)
❌ GPIO8_GDIR (direction)      - Stuck at INPUT despite write attempt
❌ GPIO8_DR (data register)    - Reads LOW
```

**Status Update (2026-01-20 01:15) - CRITICAL FINDINGS**:

## GPIO2 Works, GPIO8 Doesn't

**Breakthrough**: Found that GPIO register base addresses were wrong initially.

**Working (GPIO2 / Pin 13 LED):**
- Base address: 0x401BC000 (from imxrt.h)
- Pin 13 = GPIO2 bit 3
- LED blinks successfully with raw register access
- Proves: Basic GPIO works in Rust firmware

**Not Working (GPIO8 / Any Pin):**
- Base address: 0x42008000 (correct per imxrt.h)
- Tested: GPIO_EMC_40 (bit 26), Pin 28 (bit 18)
- Register writes succeed (GDIR/DR change correctly in software)
- Logic analyzer shows 0V flatline (no toggles)
- **Physical pins do not respond despite correct register values**
- **C++ firmware WORKS on same pins** - confirmed GPIO8 bit 18 toggles in C++
- **RUST-SPECIFIC ISSUE**: Something about Rust's GPIO8 access is broken

## Previous Issues Resolved:

~~ALL software configuration verified correct, but physical pin still outputs 0V:~~

✅ GPIO8 clock enabled (CCGR3 = 0x3)
✅ IOMUXC_SW_MUX_CTL_PAD_GPIO_EMC_40 = ALT5 (GPIO mode)
✅ IOMUXC_SW_PAD_CTL_PAD_GPIO_EMC_40 = 0x0008 (drive strength)
✅ GPIO8_GDIR bit 26 = OUTPUT
✅ GPIO8_DR bit 26 = HIGH
✅ Memory barriers added (DSB/DMB/ISB)
✅ Configuration done BEFORE BSP init
✅ Configuration also done FROM C++ (same macros as working firmware)
❌ Physical pin: 0V (measured with logic analyzer)

**Minimal working C++ test** (confirmed works on same hardware):
```c
void setup() {
  GPIO8_GDIR |= 1 << 26;
  GPIO8_DR_SET = 1 << 26;
}
```

**Next Steps**:
1. **Logic analyzer capture**: Monitor GPIO_EMC_40 from power-on through entire boot sequence
   - Does pin EVER go high, even briefly?
   - Is it being toggled/driven by something else?
   - Compare C++ vs Rust boot sequences side-by-side

2. **Linker script comparison**: Check if memory layout affects chip initialization
   - Compare `.ld` files: Rust (t4link.x) vs Arduino
   - Check FCB/IVT/DCD differences

3. **Boot sequence analysis**: What runs before `main()`?
   - Arduino: startup hooks, clock init, peripheral defaults
   - Rust: teensy4-fcb, RTIC init, BSP init

4. **MPU/Cache investigation**:
   - Check if D-Cache enabled and peripheral regions marked non-cacheable
   - Try disabling D-Cache entirely in Rust firmware

5. **Different control mechanism**:
   - Is GPIO_EMC_40 the actual control pin, or is there another signal?
   - Check Teensy 4.1 schematic for TPD3S014 wiring
   - Verify EN pin connection on actual hardware

## Test Equipment Available:

**Logic Analyzer**: Connected and verified working. Shows:
- C++ firmware: GPIO8 toggles correctly (confirmed on Pin 28 and USB VCC)
- Rust firmware: GPIO2 toggles correctly, GPIO8 shows flatline

## Debugging History:

1. **Wrong GPIO2 address** (0x42000000 → 0x401BC000) - FIXED
2. **Read-modify-write issue** - FIXED (was overwriting GDIR instead of OR-ing)
3. **SEMC disable attempted** - No effect
4. **Cache barriers added** (DSB/DMB/ISB) - No effect
5. **Clock gates verified** (CCGR3 enabled) - Confirmed working
6. **IOMUXC configuration** (MUX=5, PAD=0x10B0) - Confirmed set correctly
7. **Pre-BSP initialization** - Still no voltage output
8. **Inline assembly writes** - Testing bypass of Rust volatile writes

## Current Hypotheses (Refined):

**CRITICAL: Rust cannot access GPIO6-9 (0x4200xxxx region)**:
- GPIO2 (0x401BC000): ✅ Works in Rust
- GPIO8 (0x42008000): ✅ Works in C++, ❌ Fails in Rust
- Memory region 0x4200xxxx requires special access permissions?
- MPU (Memory Protection Unit) blocking Rust from this region?
- Different cache policy needed for 0x4200xxxx region?
- teensy4-fcb or linker script missing memory region setup?

**Possible Root Causes**:
1. **MPU Configuration**: teensy4-bsp might not configure MPU for 0x4200xxxx region
2. **Cache Policy**: 0x4200xxxx might need write-through vs write-back cache
3. **Memory Attributes**: Region needs device memory attributes, not normal memory
4. **Privilege Level**: Cortex-M7 privilege/security preventing access
5. **Linker Script**: Memory region not defined in Rust's t4link.x

**Next Steps**:
1. Flash inline-assembly test to see if ASM bypasses Rust issue
2. Check MPU configuration in Rust vs Arduino
3. Compare linker scripts (t4link.x vs Arduino)
4. Try disabling MPU entirely
5. Check if 0x4200xxxx is even mapped/accessible in Rust binary

**Diagnostic Code Added**:
- `cpp_verify_clocks()` in `sys/shim.cpp:73` - pre-init diagnostics
- `cpp_recheck_power()` in `sys/shim.cpp:155` - runtime verification every 2s
- Early GPIO8 config in `main.rs:212-226` - (not working yet)

**Files Modified**:
- `src/main.rs`: Early CCGR3 enable (line 209), attempted GPIO8 config (212-226)
- `sys/shim.cpp`: Diagnostic functions and runtime verification
- `sys/USBHost_t36/ehci.cpp`: Detailed power configuration logging

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

### Planned Features (Priority TBD)
- [ ] UI system (major pain point currently)
- [ ] Comprehensive configuration system (replace hardcoded CC20, etc.)
- [ ] Glide slide-back on note release
- [ ] Depth control for modulators
- [ ] Looping mode for animations (cycle modulation continuously)
- [ ] Additional Consumers (arpeggiators, chord generators, etc.)
- [ ] Additional Modulators (LFO, envelopes, stepped sequences, etc.)

### Known Bugs
- [ ] Bug in Glider implementation (specifics TBD)

## Development Notes

### Building & Flashing
```bash
# Build for Teensy 4.0 (thumbv7em-none-eabihf target)
cargo build --release

# Flash to Teensy (tooling TBD - check project scripts)
```

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

### Planning & Task Management Workflow

**When planning new features or initiatives:**
1. **Read workflow guides first**: Use `mcp__backlog__get_workflow_overview` and `mcp__backlog__get_task_creation_guide`
2. **Assess scope**: Determine if work is single task or needs parent/subtask structure
3. **Use parent tasks for epics**: Large initiatives (UI system, arpeggiator) should be parent tasks with subtasks
4. **Create task hierarchy**: Use `parentTaskId` parameter to link subtasks to parent
5. **Document relationships**: Use `dependencies` field to track task ordering
6. **Search first**: Always check for existing tasks with `mcp__backlog__task_search` before creating new ones
7. **Explain structure**: After creating tasks, explain the hierarchy and relationships to user

**Multi-task structure guidelines:**
- **Use subtasks** when: Multiple tasks modify same component, tightly coupled, sequential phases
- **Use separate tasks** when: Different subsystems, can be worked independently, loose coupling
- **Always create**: Parent task first, then all subtasks in same session

### DO:
- **ALWAYS create tasks in Backlog.md BEFORE starting new features** - Use `mcp__backlog__task_create` to document what you're building
- Search existing tasks first with `mcp__backlog__task_search` to avoid duplicates
- Follow the `no_std` embedded patterns
- Use `heapless` collections where possible
- Avoid heap allocations wherever possible. Remember the heap limit is 1KB.
- Respect RTIC task priorities and shared resource locking
- Log extensively for debugging (USB logging is available)
- Consider real-time constraints (MIDI timing is critical)

### DON'T:
- Start implementing new features without creating a task first
- Create tasks without reading workflow guides during planning
- Create single monolithic tasks for multi-component work
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

- `rtic`: Real-time framework (v2, thumbv7 backend)
- `teensy4-bsp`: Board support package for Teensy 4.x
- `midi-msg`: MIDI message parsing/serialization (no_std compatible)
- `heapless`: Fixed-capacity collections (Vec, Deque, etc.)
- `embedded-alloc`: Heap allocator for embedded systems
- `enum_dispatch`: Zero-cost enum dispatch for traits

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


