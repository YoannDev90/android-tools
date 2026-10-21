# svg2vd

Convert an SVG into an Android VectorDrawable XML. Pure conversion — no
device, no Android SDK, no Android Studio: the first fully pure crate
of this workspace.

Vector drawables are Android's vector format (`res/drawable/*.xml`),
and the painful part of adopting them is turning the SVG every design
tool exports into one. `svg2vd` does that conversion deterministically:
parse once with [`usvg`](https://crates.io/crates/usvg) (the parser
behind resvg), walk the tree, emit `<vector>` / `<path>` XML.

## What's converted

- Shapes and paths — `rect`, `circle`, `ellipse`, `line`, `polyline`,
  `polygon`, `path`; arcs arrive as cubic beziers, which is what
  `pathData` holds anyway
- Fills — solid color, `fill-opacity` folded into the `#AARRGGBB`
  alpha byte, `fill-rule: evenOdd` → `android:fillType`
- Strokes — color, width, linecap, linejoin, miterlimit, dasharray,
  dashoffset; renderer defaults (butt, miter 4) are omitted, not copied
- Transforms — baked into the path coordinates, so any rotation,
  skew or scale is representable (VectorDrawable `<group>` can only do
  translate+rotate+scale around a pivot)
- `clip-path` — a clipped `<g>` becomes a `<group>` with
  `<clip-path android:pathData>` before the paths it clips
- Viewport — from `width`/`height` (px become dp, 1 SVG px = 1 dp) or
  from `viewBox` alone

## What isn't (reported, never fatal)

| SVG asks for | svg2vd does | Reported |
|---|---|---|
| gradient fill/stroke | first stop color | warning |
| pattern fill/stroke | black | warning |
| group `opacity` | full opacity colors | warning |
| `filter`, `mask` | ignored | — |
| `<text>` | skipped (no font support) | — |
| `<image>` | skipped | warning |

A warning on stderr, a usable drawable out — the same deal Android
Studio's own importer offers, minus the clicking.

## Install

```bash
cargo install --path .        # from this folder
# or, from the repository root:
cargo install --path svg2vd
```

## Usage

```bash
svg2vd icon.svg                    # → icon.xml next to it
svg2vd icon.svg -o app/src/main/res/drawable/icon.xml
svg2vd - -o -                      # stdin → stdout, pipe-friendly
```

```bash
$ svg2vd triangle.svg -o -
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="24dp"
    android:height="24dp"
    android:viewportWidth="24"
    android:viewportHeight="24">
    <path
        android:pathData="M12 2 L2 22 L22 22 L12 2 Z"
        android:fillColor="#FFFF0000"/>
</vector>
```

## Development

```bash
cargo test          # pure conversion — no device needed
cargo clippy
```

Source layout: `vd.rs` (the emitter — tree walk, `pathData`
formatting, `#AARRGGBB` colors, all unit-tested against inline SVGs),
`main.rs` (CLI, file/stdin/stdout plumbing).

## License

MIT — see [LICENSE](LICENSE).
