---
id: task-7
title: Create Modulation trait and infrastructure
status: Done
assignee: []
created_date: '2025-11-19 16:12'
labels:
  - animation
  - architecture
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Design and implement the Modulation trait for defining animation behaviors with progress-based message generation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Modulation trait with animate(progress, depth, offset) method
- [ ] #2 Modulation trait with reset() method for cleanup
- [ ] #3 Modulator enum using enum_dispatch for zero-cost dispatch
- [ ] #4 Messages type alias for returning up to 3 MIDI messages
- [ ] #5 Support for depth and offset parameters (future use)
<!-- AC:END -->
