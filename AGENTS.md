# AGENTS.md

Guidelines for agents working in this repository.

## Architecture

- Keep `main.rs` minimal. It should contain the RTIC app boundary, resource declarations, task declarations or thin task wrappers, and top-level wiring only.
- Do not put complex behavior in `main.rs`. Feature logic, rendering, parsing, state machines, protocol adaptation, and policy decisions belong in named modules.
- Keep platform-specific code isolated from portable domain logic. Board support, RTIC resource types, pin types, hardware register access, and C/C++ FFI details should not leak into portable modules.
- Prefer small modules with explicit ownership over large mixed-purpose files.

## Module Layout

- `src/` should contain `main.rs` plus module directories. Do not add new root-level implementation files.
- Use the `dir/mod.rs` pattern:
  - `mod.rs` defines the public API for that module: shared traits, public structs/enums, type aliases, and deliberate `pub use` re-exports.
  - Other files inside the directory contain implementations of those traits and structs.
- Keep implementation modules private by default. Re-export only the API that other modules need.
- If a `mod.rs` grows large or contains substantial method bodies, move those implementations into a sibling file and keep `mod.rs` as the module surface.
- Name modules by domain responsibility, not by incidental hardware detail, unless the module is intentionally hardware-specific.

## RTIC Tasks

- The `#[rtic::app]` module is the app boundary. Shared/local RTIC resources and interrupt bindings are generated from declarations inside that module.
- Task bodies should be thin. A task should normally acquire RTIC resources, perform scheduling or loop mechanics required by RTIC, and delegate behavior to a module function.
- Long-running policy logic, rendering, parsing, mapping, state machines, and protocol adaptation should live outside task wrappers.
- RTIC 2.2 supports separating task signatures from implementations with `extern "Rust" { #[task(..)] fn task_name(..); }`. Use this only when it keeps the code clearer, and validate each migration with `cargo check`.
- Prefer thin in-app wrappers when generated task context types make fully external task implementations awkward.

## Embedded Constraints

- Preserve `#![no_std]` compatibility.
- Prefer `heapless` and fixed-capacity storage in timing-sensitive paths.
- Avoid heap allocations in MIDI, animation, interrupt, and dispatch paths unless there is a clear bounded reason.
- Do not block in high-priority interrupt tasks.
- Keep timing-critical code small and predictable.

## Verification

- Run `cargo check` after Rust or C/C++ wrapper changes.
- Avoid repository-wide formatting churn unless the task is explicitly a formatting cleanup.
- For hardware-facing behavior, document what was verified by build versus what still needs board testing.

## Git And Task Hygiene

- Keep commits short and tagged, matching existing style such as `[Fix] ...` or `[Feature] ...`.
- Do not include generated-tool attribution in commits.
- If adding TODOs, link them to backlog tasks with `TODO(task-#): ...` when a task exists.
- Search existing backlog tasks before adding new tracking work.
