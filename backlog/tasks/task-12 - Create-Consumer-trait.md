---
id: task-12
title: Create Consumer trait
status: Done
assignee: []
created_date: '2025-11-19 16:12'
labels:
  - architecture
  - consumer
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Define the Consumer trait for async MIDI processing that can generate multiple outputs and spawn animations.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Single method: consume(event: &MidiEvent) -> CoreOutput
- [ ] #2 CoreOutput is Vec<Output, 8> for up to 8 output messages
- [ ] #3 Output enum variants: SendMidi, Animate, BlinkLed
- [ ] #4 Consumers can process both user and synthetic messages
- [ ] #5 Consumers execute in parallel
<!-- AC:END -->
