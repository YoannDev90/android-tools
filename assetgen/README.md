# assetgen

Android density asset generator — one source PNG in, the whole density
set out: legacy launcher icons, notification icons, generic drawables,
adaptive icons, 9-patch rescales and badged debug launcher variants.

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

# Debug launcher icons: same set + a badge, into src/debug/res.
# AGP applies src/debug/res over src/main/res in debug builds — no
# Gradle change needed, release keeps the clean icons.
assetgen debug icon192.png -o app/src/debug/res --badge badge_d.svg
assetgen debug icon192.png -o app/src/debug/res --round icon_round.png \
    --badge badge.svg --badge-corner tl --badge-size 40
```

| Flag | Default | Meaning |
|------|---------|---------|
| `-o, --out` | `res` | destination directory (created when missing) |
| `-n, --name` | `ic_launcher` | resource name, `[a-z][a-z0-9_]*` |
| `--round` | — | launcher only: square PNG for the circle-cropped round icon |
| `--dp` | `24` | drawable only: dp base of the source |
| `--bg-color` / `--bg` | — | adaptive only: background as `#RGB`/`#RRGGBB`/`#AARRGGBB` or as a PNG (one required) |
| `--from` | `xhdpi` | 9patch only: density the source was authored at |
| `--badge` | — | debug only: badge artwork, `.svg` or square `.png` (required) |
| `--badge-corner` | `br` | debug only: `tl`/`tr`/`bl`/`br` |
| `--badge-size` | `33` | debug only: badge size in percent of the icon |

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

## Debug badges — how the debug-only part works

`assetgen debug` writes the *regular* launcher set, each layer carrying
a badge in the corner of your choice. Point `-o` at `src/debug/res`:

```text
app/src/main/res/mipmap-*/ic_launcher.png   ← clean icons (launcher)
app/src/debug/res/mipmap-*/ic_launcher.png  ← badged (assetgen debug)
```

The Android Gradle Plugin **merges variant source sets automatically**
(`developer.android.com/build/build-variants`): for a debug build,
`src/debug/res` is merged over `src/main/res` — same resource name, the
variant wins. So debug builds show the badge, release builds keep the
clean icons, and there is **nothing to edit in `build.gradle.kts`**: no
`sourceSets {}` block, no flavors, no DSL fragility. The only
prerequisite is that both sets declare the same resource names, which
this subcommand guarantees by design (same `-n`, same modes).

Badges:

- **SVG** — re-rendered from the vector at every density, so the badge
  stays crisp from mdpi to xxxhdpi (system fonts are loaded for text
  elements). Fit-contained and centered in its square slot.
- **PNG** — must be square; resized through the same Lanczos pipeline
  as the icons.

The badge is composited *after* downscaling, in the chosen corner, at
`--badge-size` percent of the icon.

## Development

```bash
cargo test          # 18 unit tests — density tables, name and color,
                    # rules, square enforcement, circle mask, launcher /
                    # notification / adaptive / 9-patch plans, 9-patch
                    # validation and marker scaling, badge corners /
                    # compositing / SVG render / debug plans; all in-memory,
                    # no filesystem or device needed
cargo clippy
cargo build --release
```

Source layout: `gen.rs` (the pure generation core + tests), `main.rs`
(CLI, filesystem, exit codes).

## License

MIT — see [LICENSE](LICENSE).
