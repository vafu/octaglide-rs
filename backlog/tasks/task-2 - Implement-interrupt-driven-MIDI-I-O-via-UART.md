---
id: task-2
title: Implement interrupt-driven MIDI I/O via UART
status: Done
assignee: []
created_date: '2025-11-19 16:12'
labels:
  - midi
  - hardware
  - uart
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create MidiBus abstraction for interrupt-driven MIDI RX/TX using LPUART6 at 31250 baud with buffered I/O and running status optimization.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 UART configured at 31250 baud (MIDI standard)
- [ ] #2 Interrupt-driven RX with RECEIVE_FULL interrupt
- [ ] #3 Interrupt-driven TX with TRANSMIT_EMPTY interrupt
- [ ] #4 32-byte RX/TX buffers implemented with heapless::Deque
- [ ] #5 MIDI message parsing with midi-msg crate
- [ ] #6 Running status optimization for TX to reduce bandwidth
<!-- AC:END -->
