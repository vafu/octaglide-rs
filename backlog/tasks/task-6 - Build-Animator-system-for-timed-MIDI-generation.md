---
id: task-6
title: Build Animator system for timed MIDI generation
status: Done
assignee: []
created_date: '2025-11-19 16:12'
labels:
  - animation
  - timing
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create the Animator task that manages time-based MIDI message generation using 5ms tick intervals with progress-based modulation support.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Animator runs at 5ms intervals (MSG_INTERVAL_MS)
- [ ] #2 States: Idle and Animating with progress tracking
- [ ] #3 Commands: Start, Stop, Duration
- [ ] #4 Progress calculation based on elapsed time
- [ ] #5 Integration with Systick monotonic timer
- [ ] #6 Async tick loop with biased select for command vs timing
<!-- AC:END -->
