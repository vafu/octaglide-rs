---
id: task-29
title: Event-driven OLED redraw via Core change notifications
status: To Do
assignee: []
created_date: '2026-03-04 07:43'
updated_date: '2026-03-04 07:45'
labels:
  - display
  - power
  - optimization
dependencies: []
priority: high
ordinal: 500
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Currently `oled_task` polls CONFIGS atomics every 16ms and skips redraw when values are unchanged. This still wastes CPU cycles on every tick.

**Desired behavior**: OLED task sleeps until notified of a parameter change, then redraws once.

**Approach**: When the Core (or a Transformer/Consumer) updates an envelope parameter, signal the OLED task via a channel or RTIC signal. The OLED task blocks on `recv()` instead of polling.

Options:
- Add a `rtic_sync` channel `()` (capacity 1) that acts as a "dirty" wake signal — senders use `try_send` (drop if already pending), receiver awaits it
- Or use an `AtomicBool` dirty flag + Waker, though the channel approach is cleaner in RTIC

This eliminates all idle CPU use and I2C traffic when parameters aren't changing (important for battery-powered use).
<!-- SECTION:DESCRIPTION:END -->
