---
id: task-19
title: Implement chord mode
status: To Do
assignee: []
created_date: '2025-11-19 16:51'
updated_date: '2026-03-04 07:45'
labels:
  - chord
  - feature
  - music-theory
dependencies: []
priority: medium
ordinal: 4000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Transform single input notes into full chords with configurable voicing, quality, and inversions. Each chord note outputs on a different MIDI channel (configurable).

Supports two modes:
1. Manual mode: User-selected chord quality applied to all input notes (e.g., A minor, B minor, C minor)
2. Quantized mode: Chord qualities automatically determined by scale and scale degree (e.g., in A minor: A=i (minor), C=III (major))

This is a complex feature that interacts with the quantizer and trigger systems.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Single input note produces multiple output notes
- [ ] #2 Each output note sent on different MIDI channel
- [ ] #3 MIDI channels configurable
- [ ] #4 Manual mode: user-selected chord quality applied consistently
- [ ] #5 Quantized mode: chord quality determined by scale and degree
- [ ] #6 Supports chord voicing configuration
- [ ] #7 Supports inversion settings
- [ ] #8 Integrates with quantizer when quantizer mode enabled
<!-- AC:END -->
