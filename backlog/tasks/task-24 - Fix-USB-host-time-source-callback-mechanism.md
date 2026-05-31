---
id: task-24
title: Fix USB host time source callback mechanism
status: To Do
assignee: []
created_date: '2026-01-25 20:49'
updated_date: '2026-03-04 07:45'
labels:
  - bug
  - usb
  - investigation
dependencies: []
priority: low
ordinal: 9000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The USB host time source callback (rust_micros) works when called directly from main.rs, but breaks USB enumeration when implemented via a callback indirection in the teensy-usbhost crate.

**Current situation:**
- `rust_micros()` defined in teensy-usbhost crate calls a closure stored in static mut
- Time source callback itself works (verified with logs)
- First call to rust_micros from C++ succeeds
- USB enumeration completely fails after initialization

**Working approach:**
- `rust_micros()` defined directly in main.rs calling Systick
- USB enumeration works perfectly

**Investigation needed:**
- Why does callback indirection break USB enumeration?
- Is it timing-related? (closures add overhead)
- Is it ABI/calling convention related?
- Is it optimization-related?

**Files:**
- teensy-usbhost/src/lib.rs - callback mechanism
- src/main.rs - working direct implementation
<!-- SECTION:DESCRIPTION:END -->
