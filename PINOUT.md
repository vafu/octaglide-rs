# OctaglideRS Pin Assignment

Verified against the X2 component and net attributes in the assembled board's
`kicad_test_ai-B_Cu.gbr` fabrication file (generated 2026-08-30).

| Pin | Function | Peripheral / GPIO | Interrupt | PCB net / notes |
|-----|----------|-------------------|-----------|-----------------|
| P7 | Reset button | GPIO2_IO17 | GPIO2_COMBINED_16_31 | `P7_RESET`; active low |
| P13 | Status LED | GPIO2_IO03 | - | Teensy onboard LED; not routed by carrier PCB |
| P18 | OLED SDA | LPI2C1 SDA | - | `DS1-SDA`; I2C at 400 kHz |
| P19 | OLED SCL | LPI2C1 SCL | - | `DS1-SCL`; I2C at 400 kHz |
| P20 | MIDI UART TX | LPUART8 TX | LPUART8 | `RX_FROM_TEENSY_TX` at MIDI connector |
| P21 | MIDI UART RX | LPUART8 RX | LPUART8 | `TX_TO_TEENSY_RX` at MIDI connector |
| P24 | Release slider | ADC1 input 1 | - | `P24_REL`; 10-bit ADC |
| P25 | Sustain slider | ADC1 input 2 | - | `P25_SUS`; 10-bit ADC |
| P26 | Decay slider | ADC2 input 3 | - | `P26_DEC`; 10-bit ADC |
| P27 | Attack slider | ADC2 input 4 | - | `P27_ATT`; 10-bit ADC |
| P28 | Button 1 | GPIO3_IO18 | GPIO3_COMBINED_16_31 | `P28_BTN1`; active low; debounced press event 0 |
| P29 | Button 2 | GPIO4_IO31 | GPIO4_COMBINED_16_31 | `P29_BTN2`; active low; debounced press event 1 |
| P30 | Button 3 | GPIO3_IO23 | GPIO3_COMBINED_16_31 | `P30_BTN3`; active low; debounced press event 2 |
| P31 | Button 4 | GPIO3_IO22 | GPIO3_COMBINED_16_31 | `P31_BTN4`; active low; debounced press event 3 |
| P39 | Encoder A | GPIO1_IO29 | GPIO1_COMBINED_16_31 | `P39_ENC_A`; either edge |
| P40 | Encoder B | GPIO1_IO20 | GPIO1_COMBINED_16_31 | `P40_ENC_B`; either edge |
| P41 | Encoder click | GPIO1_IO21 | GPIO1_COMBINED_16_31 | `ENC1-SW`; both edges; debounced pressed/released |

## PCB wiring

- Sliders are ordered left-to-right as Attack, Decay, Sustain, Release on P27, P26, P25, and P24. Each wiper is RC-filtered before the ADC input.
- OLED DS1 connects SDA to P18 and SCL to P19.
- Encoder A, B, and click connect to P39, P40, and P41. All three signals have external pullups to 3.3 V and capacitors to ground.
- The reset switch connects P7 to ground and relies on the MCU's internal pullup.
- Buttons 1-4 connect P28-P31 to ground and have external pullups and capacitors.
- Firmware observes both edges, emits one pressed event, then one released event after a stable 30 ms release. The button task sleeps between edges, with short timer checks only during a pending release.
- The encoder switch uses the same pressed/released tracking as the four buttons. Hold it, press button 1-4, and release that button while still holding the encoder to select the corresponding track. Releasing the encoder first cancels selection. Rotation edits the selected consumer. Clicking alone has no effect. A/D/R sliders edit durations normally and curves while the encoder is held; sustain always edits its level.
- Without the encoder modifier, clicking/releasing button 1-4 selects modulation lane 1-4. Each voice has exactly four lanes: passthrough, passthrough, envelope 1 (CC2), envelope 2 (CC3). The header shows L1-L4; passthrough lanes show their label rather than an envelope. Musical MIDI processes octave/glide first, then all four lanes; hardware edits target the selected lane.
- Envelope duration and sustain pickup prevents jumps: after startup, changing a slider mapping, or selecting another track/voice/envelope, edits wait until the slider reaches or crosses that parameter's stored value. After pickup it follows normally until the next mapping or selection change. Curve edits bypass pickup and respond immediately while the encoder is held.
- Curve slider direction is logarithmic at the top and exponential at the bottom, with a linear center. The OLED preview and generated envelope use the same direction.
- OLED layout: track/voice/lane is right-aligned at the top, USB status U is at the top left, and envelope mode AD/AR/ADSR is centered at the bottom with its own text row below the graph.
- Envelope previews use a fixed horizontal duration scale across AD, AR and ADSR, with room for all three maximum ramps. Shorter envelopes leave baseline space on the right; changing one stage does not rescale the others. Zero-length ramps are instantaneous and positive ramps get at least one visible pixel. Sustain hold is an illustrative plateau, not a prediction of note length.

## Unrouted Teensy pins

The carrier PCB marks P0-P6, P8-P12, P14-P17, P22-P23, and P32-P38 as not connected. Teensy 4.1 underside pins P42-P54 are not broken out by this carrier.
