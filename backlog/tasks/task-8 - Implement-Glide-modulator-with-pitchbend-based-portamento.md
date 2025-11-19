---
id: task-8
title: Implement Glide modulator with pitchbend-based portamento
status: Done
assignee: []
created_date: '2025-11-19 16:12'
labels:
  - glide
  - modulator
  - feature
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create the Glide modulator that smoothly transitions between notes using pitchbend, intelligently switching active notes to stay within synth bend range.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Calculate interpolated note position based on progress
- [ ] #2 Determine which physical note should be active (from, to, or intermediate)
- [ ] #3 Switch active notes when out of pitchbend range (±2 semitones assumed)
- [ ] #4 Send NoteOn for new active note and NoteOff for previous note on switch
- [ ] #5 Generate PitchBend messages to achieve precise pitch
- [ ] #6 Reset pitchbend to center on animation end
- [ ] #7 All messages marked as synthetic
<!-- AC:END -->
