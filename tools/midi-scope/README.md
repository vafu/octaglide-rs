# midi-scope

Rolling MIDI monitor. Attaches to a MIDI output port and plots one control
change over a 10-second window.

```bash
cargo run --release --manifest-path tools/midi-scope/Cargo.toml -- \
  --cc 3 --channel 1 --color ff8800
```

Options:

| Flag | Meaning |
|---|---|
| `--cc <0-127>` | Control change to plot (required unless `--notes`) |
| `--channel <1-16>` | MIDI channel (default 1) |
| `--color <RRGGBB>` | Line colour, `#` optional (default `ffffff`) |
| `--port <NAME>` | Port substring to attach to (default `octaglide`) |
| `--notes` | Note view (not implemented yet) |
| `--quiet` | Do not echo incoming messages to the terminal |

Every message received is echoed to stdout as it arrives, e.g.
`ch 1 CC 3 = 42` or `ch 1 note on  60 vel 100`. Use `--quiet` to silence it.

Run one instance per CC to watch several at once.

## Wayland note

eframe picks the native Wayland backend by default. `xdotool` and
screen-capture tools that go through X cannot see those windows. Either use
your compositor's own screenshot tooling, or force XWayland:

```bash
WAYLAND_DISPLAY= cargo run --release --manifest-path tools/midi-scope/Cargo.toml -- --cc 3
```
