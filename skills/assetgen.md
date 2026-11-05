# assetgen

Android density asset generator — one source PNG in, the whole density set out: legacy launcher icons, notification icons, generic drawables, adaptive icons, 9-patch rescales and badged debug launcher variants.

## When to use

- Reach for `assetgen` when its dedicated purpose matches the task (see description above).
- Prefer it over hand-rolled adb/shell commands for the features it wraps.

## Common commands

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

## Notes

- Run `assetgen --help` for the full option list.
- Device-touching commands need `adb` on PATH and an authorized device unless noted.
