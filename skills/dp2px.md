# dp2px

px ↔ dp ↔ sp converter for Android — offline, or against the density your device actually draws at.

## When to use

- Reach for `dp2px` when its dedicated purpose matches the task (see description above).
- Prefer it over hand-rolled adb/shell commands for the features it wraps.

## Common commands

```bash
dp2px 1080                              # 1080 px → dp at the default 160 dpi (mdpi)
dp2px 1080 --device                     # …at the connected device's real density
dp2px 48 --from dp --to px --density 440  # 48 dp at 440 dpi → px
dp2px 48 --from dp --device             # 48 dp on your device → px
dp2px 48 72 96 --from dp --to px --device  # several values, one result per line
dp2px 16 --from sp --to px --font-scale 1.15  # sp with a custom font scale
dp2px 1080 --device --font-scale 1.3    # …sp result honours the font scale too
```

`-s, --serial <SERIAL>` selects a device; with exactly one device
connected it is picked automatically.

### Flags

| Flag | Default | Meaning |
|------|---------|---------|
| `--from px\|dp\|sp` | `px` | Unit the values are in |
| `--to px\|dp\|sp` | `dp` | Unit to convert to |
| `-d, --density <dpi>` | `160` | Screen density in dpi (mdpi baseline). Conflicts with `--device` |
| `--device` | — | Read the density from `wm density` (override wins over physical) |
| `--font-scale <f>` | `1.0` | Font scale applied to `sp` conversions |
| `-s, --serial <SERIAL>` | auto | adb serial |

## Notes

- Run `dp2px --help` for the full option list.
- Device-touching commands need `adb` on PATH and an authorized device unless noted.
