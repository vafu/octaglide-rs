---
id: task-3
title: Implement USB logging system
status: Done
assignee: []
created_date: '2025-11-19 16:12'
labels:
  - infrastructure
  - debugging
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Set up USB-based logging using imxrt-log for debugging MIDI stream processing on the device.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 USB logging configured with imxrt-log
- [ ] #2 log::info!() and log::error!() work correctly
- [ ] #3 USB_OTG1 interrupt handler polls logger
- [ ] #4 MIDI messages logged with <<< (incoming) and >>> (outgoing) prefixes
- [ ] #5 PitchBend messages excluded from logs to reduce noise
<!-- AC:END -->
