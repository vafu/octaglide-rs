---
id: task-23
title: Build user-configurable parameter mapping system
status: To Do
assignee: []
created_date: '2025-11-19 16:51'
updated_date: '2026-03-04 07:45'
labels:
  - configuration
  - parameters
  - feature
dependencies:
  - task-16
priority: low
ordinal: 8000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create comprehensive configuration system allowing users to map MIDI CC numbers to any parameter control. Replace all hardcoded CC mappings (like CC20 for octave shift) with user-configurable assignments.

This enables users to customize the device for their specific controller and workflow. Configuration should be persistent (stored in EEPROM/Flash).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Users can map any CC number to any parameter
- [ ] #2 All hardcoded CC mappings replaced with configurable system
- [ ] #3 Configuration persists across power cycles (EEPROM/Flash)
- [ ] #4 Configuration editable via hardware UI or MIDI SysEx
- [ ] #5 Default mappings provided for common controllers
- [ ] #6 Easy to extend for new parameters
<!-- AC:END -->
