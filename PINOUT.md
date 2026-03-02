# OctaglideRS Pin Assignment

| Pin | Function         | GPIO         | Interrupt              | Notes                              |
|-----|-----------------|--------------|------------------------|------------------------------------|
| P0  | MIDI UART TX    | -            | -                      | LPUART6 TX → synth                 |
| P1  | MIDI UART RX    | -            | -                      | LPUART6 RX ← Octatrack             |
| P13 | Status LED      | GPIO2        | -                      | Board LED                          |
| P14 | ADC Slider 0    | ADC1 ch7     | -                      | Attack  (A0, 0-1023 → 0-127)       |
| P15 | ADC Slider 1    | ADC1 ch8     | -                      | Decay   (A1, 0-1023 → 0-127)       |
| P16 | ADC Slider 2    | ADC1 ch12    | -                      | Sustain (A2, 0-1023 → 0-127)       |
| P17 | ADC Slider 3    | ADC1 ch11    | -                      | Release (A3, 0-1023 → 0-127)       |
| P18 | Encoder Click   | GPIO1_IO17   | GPIO1_COMBINED_16_31   | Falling edge                       |
| P19 | Encoder B       | GPIO1_IO16   | -                      | Polled in encoder ISR, no interrupt|
| P20 | Encoder A       | GPIO1_IO26   | GPIO1_COMBINED_16_31   | Falling edge → read B for direction|
| P33 | Reset Button    | GPIO4_IO07   | GPIO4_COMBINED_0_15    | Rising edge, resets MCU            |

## Wiring

- **Sliders**: wiper to pin, one end to GND, other end to 3.3V (internal 100kΩ pulldown)
- **Encoder A/B**: EC11 common → GND, A → P18, B → P19 (internal 100kΩ pullup, falling edge)
- **Encoder click**: EC11 SW pins → P20 and GND (internal 100kΩ pullup, falling edge)
- **Reset button**: one leg to P33, other leg to 3.3V (internal 100kΩ pulldown, rising edge)

Recommended: 10–100nF cap from each encoder signal pin to GND for hardware debounce.

## Free Pins (notable)

- P2–P12, P21–P32, P34–P41: unassigned
- ADC capable: P14–P27 (A0–A13)
- ACMP capable: dedicated comparator pins (see task-25 for planned migration)
