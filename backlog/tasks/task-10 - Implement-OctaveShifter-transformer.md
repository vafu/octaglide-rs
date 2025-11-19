---
id: task-10
title: Implement OctaveShifter transformer
status: Done
assignee: []
created_date: '2025-11-19 16:12'
labels:
  - transformer
  - feature
  - octave-shift
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create a transformer that shifts notes by octaves based on CC20 control values, mapping 0-127 to octave shifts.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Listen for CC20 messages (hardcoded, to be configurable later)
- [ ] #2 Map CC value 0-127 to octave shifts: (value - 64) / 16 * 12 semitones
- [ ] #3 Range: -4 to +3 octaves
- [ ] #4 Apply offset to all NoteOn/NoteOff messages
- [ ] #5 Clamp results to valid MIDI range (0-127)
- [ ] #6 Filter out CC20 messages (return None)
<!-- AC:END -->
