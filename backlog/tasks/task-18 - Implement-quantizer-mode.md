---
id: task-18
title: Implement quantizer mode
status: To Do
assignee: []
created_date: '2025-11-19 16:51'
labels:
  - quantizer
  - feature
  - music-theory
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add quantizer that constrains incoming MIDI notes to a selected musical scale. User can choose scale type and root note. All input notes are quantized to nearest note in the scale.

This is foundational for advanced chord mode functionality where chord qualities automatically adapt to the scale.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 User can select scale type (major, minor, modes, etc.)
- [ ] #2 User can select root note
- [ ] #3 Input notes quantized to nearest scale degree
- [ ] #4 Quantization rules available for chord mode to use
- [ ] #5 Scale selection controllable via parameter system
<!-- AC:END -->
