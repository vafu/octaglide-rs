---
id: task-9
title: Implement Glider consumer for note tracking and glide triggering
status: Done
assignee: []
created_date: '2025-11-19 16:12'
labels:
  - glide
  - consumer
  - feature
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create the Glider consumer that tracks up to 8 held notes and triggers glide animations when notes overlap.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Track up to 8 held notes (MAX_HELD_NOTES)
- [ ] #2 Trigger glide animation when new note played while another is held
- [ ] #3 Send direct NoteOn for first note (no previous note to glide from)
- [ ] #4 Handle note release with slide-back to previous held note
- [ ] #5 Stop animation when all notes released
- [ ] #6 Filter synthetic NoteOff messages for user-held notes
- [ ] #7 Pass through synthetic NoteOn and PitchBend messages
<!-- AC:END -->
