---
id: task-27
title: Add physical ADSR sliders via ADC (pins 14-17)
status: Done
assignee: []
created_date: '2026-03-02 03:58'
labels:
  - feature
  - hardware
  - envelope
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Wired 4 physical sliders to ADC pins P14-P17 (A0-A3) for hardware ADSR control.

- Reset button moved from P14 to P33 (GPIO4_IO07) to free up ADC pins
- New `Input::AnalogUpdate { index, value }` variant routes hardware reads through Core
- `read_sliders` task polls ADC every 50ms, sends updates only when value changes by >4 counts (deadband noise rejection)
- Core maps slider index 0-3 to Attack/Decay/Sustain/Release in CONFIGS[0]
- ADC resolution: 10-bit (0-1023) mapped to 0-127
<!-- SECTION:DESCRIPTION:END -->
