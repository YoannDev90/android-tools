# dp2px

px ↔ dp ↔ sp converter for Android — offline, or against the density
your device actually draws at.

The arithmetic every Android dev ends up redoing in their head (or in a
browser tab): how many pixels is that 48 dp touch target, and how many
dp is this 1080 px screenshot really. `dp2px` answers both, and with
`--device` it asks the device instead of assuming a density — the
`wm density` override wins over the physical number, because that is
what the UI draws at.

## Requirements

- None for offline conversion
- `adb` (Android SDK platform-tools) in `PATH` and a connected device
  only for `--device`

## Install

```bash
cargo install --path .        # from this folder
# or, from the repository root:
cargo install --path dp2px
```

## Usage

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

## Method notes

- 160 dp is the baseline width, so `px = dp × density / 160` (mdpi 160
  = 1:1, xxhdpi 480 = 3:1). Everything routes through px, the only unit
  the screen physically has.
- `sp` is dp's typographic sibling: the same density factor, times the
  font scale. The scale is a plain ratio between the two: it drops out
  of `px ↔ dp`, and `sp → dp` multiplies by it (16 sp at scale 1.5
  occupies the space of 24 dp).
- `--device` reads `wm density` and prefers the **override** density
  when there is one — an app sized against the physical number would be
  wrong on any device with display scaling.
- Results print with at most two decimals, trailing zeros dropped
  (`392.73`, `132`). A non-positive font scale or a zero density is an
  error, not a silent division by zero.

## Development

```bash
cargo test          # pure conversion core — no device needed
cargo clippy
cargo build --release
```

Source layout: `main.rs` (CLI and device wiring), `convert.rs` (pure
conversion core and formatting). Depends on [`adbkit`](../adbkit/) only
for `--device` density lookup.

## License

MIT — see [LICENSE](LICENSE).
