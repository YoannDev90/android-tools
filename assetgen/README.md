# assetgen

Android density asset generator — one source PNG in, the whole density
set out: legacy launcher icons, notification icons, generic drawables,
adaptive icons and 9-patch rescales.

The offline equivalent of Android Asset Studio / the 9-Patch Resizer:
same density ladder (mdpi → xxxhdpi), same Lanczos-quality downscaling,
no network, no SDK.

## Requirements

- None at runtime — pure PNG processing, no device, no SDK.
- The source should be **square** (refused otherwise) and ideally cover
  the xxxhdpi target; smaller sources still work, the CLI warns before
  upscaling.

## Install

```bash
cargo install --path .        # from this folder
# or, from the repository root:
cargo install --path assetgen
```

## Usage

```bash
# Legacy launcher icons: mipmap-*/ic_launcher.png at 48 dp.
assetgen launcher icon192.png -o app/src/main/res
assetgen launcher icon192.png -o app/src/main/res --round icon_round.png

# Notification icons: drawable-*/ic_stat_paste.png at 24 dp.
assetgen notification paste24.png -o app/src/main/res -n ic_stat_paste

# Any drawable at a dp base of your choice.
assetgen drawable hero.png -o app/src/main/res -n bg_hero --dp 100

# Adaptive icon: 108 dp foreground + background + mipmap-anydpi-v26 XMLs.
assetgen adaptive fg432.png -o app/src/main/res --bg-color '#2196F3'
assetgen adaptive fg432.png -o app/src/main/res --bg background.png

# Rescale an existing .9.png across the other densities.
assetgen 9patch btn.9.png -o app/src/main/res -n btn_row --from xhdpi
```

| Flag | Default | Meaning |
|------|---------|---------|
| `-o, --out` | `res` | destination directory (created when missing) |
| `-n, --name` | `ic_launcher` | resource name, `[a-z][a-z0-9_]*` |
| `--round` | — | launcher only: square PNG for the circle-cropped round icon |
| `--dp` | `24` | drawable only: dp base of the source |
| `--bg-color` / `--bg` | — | adaptive only: background as `#RGB`/`#RRGGBB`/`#AARRGGBB` or as a PNG (one required) |
| `--from` | `xhdpi` | 9patch only: density the source was authored at |

### Exit codes

| Code | Meaning |
|------|---------|
| `0` | Assets generated |
| `2` | Error (unreadable/oversized input, invalid name, non-square source, unknown density, both background flags…) |

## Output

One line per written file, `path widthxheight`:

```text
mipmap-xxxhdpi/ic_launcher.png 192x192
mipmap-xxxhdpi/ic_launcher_round.png 192x192
```

Sizes follow the platform ladder: launcher 48/72/96/144/192 px,
notification 24/36/48/72/96 px, adaptive layers 108/162/216/324/432 px,
generic drawables `--dp × density`.

## Method notes

- **Rounding** — `dp → px` rounds half up, like the platform.
  A 27 dp asset at hdpi is 40.5 → 41 px.
- **Filtering** — Lanczos3 in both directions, the same quality bar as
  Android Asset Studio; upscaling is allowed (with a warning) since
  small sources are common.
- **Round icons** — circle-cropped from the square source (Asset Studio
  behavior), corners transparent.
- **Adaptive** — foreground/background layers land in `drawable-*`, the
  v26 entry points in `mipmap-anydpi-v26` (both `<name>.xml` and
  `<name>_round.xml`); a color background is emitted as a
  `<color name="<name>_background">` resource.
- **9-patch** — the source must be a real `.9.png`: 1 px transparent
  border, black stretch markers on top and left, bottom/right blank or
  mirroring exactly (the aapt contract — validated, broken patches are
  refused). On rescale, content goes through Lanczos, borders stay 1 px
  and the markers are repainted at their scaled positions on all four
  sides; a marker never shrinks below one pixel.
- Marker positions are content-space: the 1 px border offset is removed
  on parse and re-added on write.

## Development

```bash
cargo test          # 12 unit tests — density tables, name and color
                    # rules, square enforcement, circle mask, launcher /
                    # notification / adaptive / 9-patch plans, 9-patch
                    # validation and marker scaling; all in-memory, no
                    # filesystem or device needed
cargo clippy
cargo build --release
```

Source layout: `gen.rs` (the pure generation core + tests), `main.rs`
(CLI, filesystem, exit codes).

## License

MIT — see [LICENSE](LICENSE).
