# scenediff

Visual regression diff of two screenshots — catch what changed on screen,
in CI or by hand.

Compares two images pixel by pixel: how many pixels differ, in what
percentage, and where (bounding box). Optionally writes a red-marked
overlay image, and exits 1 above a `--threshold` percentage so it works
as a CI gate. The two inputs are files, or two live captures from a
device.

## Requirements

- None for file mode
- `adb` (Android SDK platform-tools) in `PATH` and a connected device
  only for `--device`

## Install

```bash
cargo install --path .        # from this folder
# or, from the repository root:
cargo install --path scenediff
```

## Usage

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

## Output

```text
100 of 10000 pixels differ (1.000%) — bbox [20,30][29,39]
threshold 0.500%: FAIL
```

`--json` prints the same report as an object (`a`, `b`, `width`,
`height`, `compared`, `differing`, `percent`, `bbox`, `out`, `threshold`,
`failed`), with `bbox` as `[x1, y1, x2, y2]` (inclusive) or `null` when
the images are identical.

`-o diff.png` writes the overlay: image A with every differing pixel in
red, plus a two-pixel red frame around the bounding box. With `--region`
the overlay is the crop — exactly the compared area, at compared-image
coordinates.

## Method notes

- Pixels are compared on their **RGB** triple (Euclidean distance,
  squared so it stays integer). Alpha is ignored: a screenshot pair
  never legitimately differs by alpha alone. Any change in any channel
  counts — there is no per-pixel tolerance, which keeps the verdict
  deterministic.
- The percentage is over the **compared** area: with `--region`, the
  crop, not the full image.
- `--region x1,y1,x2,y2` clamps to the image like `uitool shot --crop`;
  a region that ends up empty is an error, not a silently empty pass.
- `--device` takes both captures with `exec-out screencap -p`
  (`--delay`, default 1000 ms apart) — animations and transitions make
  device diffs noisy, so crop them out or settle the screen first.

## Development

```bash
cargo test          # 16 unit tests — diff core, crop, overlay, report; no device needed
cargo clippy
cargo build --release
```

Source layout: `main.rs` (CLI and wiring), `diff.rs` (pure core: pixel
comparison, crop, overlay), `output.rs` (text/JSON report). Depends on
[`adbkit`](../adbkit/) for the `--device` captures.

## License

MIT — see [LICENSE](LICENSE).
