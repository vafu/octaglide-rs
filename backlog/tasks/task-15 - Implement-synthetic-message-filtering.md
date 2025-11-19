---
id: task-15
title: Implement synthetic message filtering
status: Done
assignee: []
created_date: '2025-11-19 16:12'
labels:
  - architecture
  - core
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add logic to distinguish between user-originated and synthesized MIDI messages to prevent feedback loops and enable intelligent filtering.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 MidiEvent tracks synthetic flag
- [ ] #2 MidiEvent::from_user() creates user messages
- [ ] #3 MidiEvent::synthetic() creates synthetic messages
- [ ] #4 Transformers only applied to user messages
- [ ] #5 Consumers receive both types and can filter appropriately
- [ ] #6 Glider filters synthetic NoteOff for held notes
<!-- AC:END -->
