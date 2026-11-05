# scenediff

Visual regression diff of two screenshots — catch what changed on screen, in CI or by hand.

## When to use

- Reach for `scenediff` when its dedicated purpose matches the task (see description above).
- Prefer it over hand-rolled adb/shell commands for the features it wraps.

## Common commands

```bash
scenediff before.png after.png               # any change fails (threshold 0)
scenediff a.png b.png --threshold 0.5        # allow up to 0.5% of pixels
scenediff a.png b.png -o diff.png            # write the overlay image
scenediff a.png b.png --region 0,80,1080,2400  # ignore the status bar
scenediff a.png b.png --json                 # machine-readable report
scenediff --device                           # capture twice, then diff
scenediff --device --delay 3000              # …3 s apart (a transition)
```

`-s, --serial <SERIAL>` selects a device; with exactly one device
connected it is picked automatically.

### Exit codes

| Code | Meaning |
|------|---------|
| `0` | Within the threshold (identical images always pass) |
| `1` | Differing percentage strictly above the threshold — the CI gate |
| `2` | Error (unreadable file, size mismatch, no device…) |

Errors never share exit 1: a broken command must not read as a visual
regression.

## Notes

- Run `scenediff --help` for the full option list.
- Device-touching commands need `adb` on PATH and an authorized device unless noted.
