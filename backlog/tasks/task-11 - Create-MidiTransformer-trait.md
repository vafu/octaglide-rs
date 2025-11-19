---
id: task-11
title: Create MidiTransformer trait
status: Done
assignee: []
created_date: '2025-11-19 16:12'
labels:
  - architecture
  - transformer
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Define the MidiTransformer trait for synchronous, in-place MIDI message modification with filtering support.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Single method: process(msg: MidiMsg) -> Option<MidiMsg>
- [ ] #2 Return Some(msg) to pass through or modify
- [ ] #3 Return None to filter out message
- [ ] #4 Applied serially in defined order
- [ ] #5 Only applied to user MIDI (not synthetic)
<!-- AC:END -->
