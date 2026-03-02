---
id: task-25
title: Move sliders to ACMP comparator pins for interrupt-driven change detection
status: To Do
assignee: []
created_date: '2026-03-02 03:55'
labels:
  - performance
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Currently the 4 ADC sliders (P14-P17, ADSR) are polled every 50ms. The IMXRT1060 has 4 ACMP (analog comparator) channels (ACMP1-4) which could replace polling with true interrupt-driven detection.

Use a tracking comparator pattern:
1. Route sliders to ACMP-capable pins
2. On ACMP interrupt: read new ADC value, reprogram comparator threshold to new_value ± hysteresis
3. CPU sleeps until actual slider movement — no periodic polling needed

Benefits: instant response, zero CPU overhead when sliders are idle, better for battery-powered scenarios.
<!-- SECTION:DESCRIPTION:END -->
