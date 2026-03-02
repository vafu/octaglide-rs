---
id: task-28
title: Upgrade encoder to full quadrature decoding
status: To Do
assignee: []
created_date: '2026-03-02 04:08'
labels:
  - hardware
  - encoder
  - performance
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Currently the encoder uses simple single-interrupt decoding (interrupt on A rising edge, read B for direction). This misses steps under fast rotation and is more susceptible to contact bounce.

Full quadrature decoding:
- Interrupt on both edges of both A and B pins (4 state transitions per detent)
- Track (prev_AB, curr_AB) through a lookup table → +1, -1, or 0 (invalid/bounce)
- Invalid states from bounce are naturally filtered out
- Catches every step even at high speed

This matters more as the encoder gains roles beyond mode cycling (parameter control, menu navigation).
<!-- SECTION:DESCRIPTION:END -->
