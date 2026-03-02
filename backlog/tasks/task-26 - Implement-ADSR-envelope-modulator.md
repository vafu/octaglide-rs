---
id: task-26
title: Implement ADSR envelope modulator
status: Done
assignee: []
created_date: '2026-03-02 03:58'
labels:
  - feature
  - envelope
  - modulator
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implemented a multi-stage envelope modulator with support for three modes:
- AD (Attack → Decay)
- AR (Attack → Hold → Release)
- ADSR (Attack → Decay → Hold → Release)

Envelope outputs CC messages at configurable channel/CC number. Hold stage waits until all notes are released before advancing. Parameters (attack, decay, sustain, release, mode) stored as AtomicU8 in static CONFIGS array for lock-free access from multiple tasks.

ModTrigger consumer fires the envelope on NoteOn and accepts CC1-5 to update parameters at runtime.
<!-- SECTION:DESCRIPTION:END -->
