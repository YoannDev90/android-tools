# iconkit

Search and download Material Symbols icons
([fonts.google.com/icons](https://fonts.google.com/icons)) from the
command line: one catalog, cached locally, searched offline — then the
SVG you wanted, or a ready-to-use Android VectorDrawable.

Google publishes the whole set behind two endpoints, and `iconkit`
speaks both: the official metadata JSON (`categories`, `tags`,
`popularity`, available sizes for every icon) and the static SVG each
style serves. The catalog is downloaded once and cached under
`$XDG_CACHE_HOME/iconkit`, so searching 3 932 icons is instant,
offline, and never a network round-trip — only `get` downloads, and
`--refresh` re-fetches the catalog when the set changes.

## What it does

- **Search** — every word must match the icon's name, a tag or a
  category; exact names outrank prefixes, prefixes outrank substrings,
  popularity breaks ties
- **Filter** — `--category` / `--tag`, repeatable and OR-ed (the two
  filters are AND-ed), case-insensitive both sides
- **Four styles** — `filled`, `outlined` (default), `rounded`,
  `sharp`; repeat `--style` to search or download several at once
- **Download SVG** — exactly what fonts.google.com/icons serves, at
  the icon's own size (20/24/40/48 px; a wrong size fails with the
  list, never a 404)
- **Download VectorDrawable** — `--vd` runs the SVG through the
  workspace's `svg2vd` emitter: `ic_alarm.xml`, valid
  `res/drawable/*.xml`, Android-safe resource names (`13mp` →
  `ic_13mp`), warnings (gradients, opacity) on stderr, never fatal
- **Inspect** — `info` for one icon (categories, tags, sizes, direct
  URL), `categories` for the whole vocabulary with counts

## Install

```bash
cargo install --path .        # from this folder
# or, from the repository root:
cargo install --path iconkit
```

## Usage

```bash
iconkit search alarm                      # best matches first
iconkit search clock --category Actions   # filter by category
iconkit search timer --tag countdown      # filter by tag
iconkit search clock --json               # machine-readable hits
iconkit categories                        # the category vocabulary
iconkit info alarm                        # tags, sizes, direct SVG URL

iconkit get alarm                         # alarm.svg
iconkit get alarm --vd                    # ic_alarm.xml (VectorDrawable)
iconkit get alarm 13mp --vd -o app/src/main/res/drawable
iconkit get settings --style rounded --style sharp   # one file per style
iconkit get alarm --size 48 --prefix "" -o icons     # 48 px, raw name
iconkit --refresh search alarm            # re-download the catalog
```

```bash
$ iconkit info alarm
alarm — Material Symbols Outlined
  categories  Actions
  tags        access, adjust, alarm, alert, appointment, bell, calendar, circle, clock, control +20
  popularity  4882
  sizes px    20, 24, 40, 48
  svg         https://fonts.gstatic.com/s/i/short-term/release/materialsymbolsoutlined/alarm/default/24px.svg
```

```bash
$ iconkit get alarm --vd
wrote ./ic_alarm.xml
```

With several `--style`s, file names carry the style
(`ic_alarm_outlined.xml`, `ic_alarm_sharp.svg`, …).

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | everything requested was found and written |
| 2 | at least one icon was missing, or an error occurred |

`get` still writes the icons that worked — misses are reported on
stderr and raise the code, so a typo never cancels the downloads.

## Development

```bash
cargo test          # pure: parsing, ranking, naming — no device, no network
cargo clippy
```

Source layout: `catalog.rs` (metadata parsing, cache, the two URL
builders), `search.rs` (ranking, filters, Android resource names),
`net.rs` (the only HTTP), `main.rs` (CLI, output, exit codes).

## License

MIT — see [LICENSE](LICENSE).
