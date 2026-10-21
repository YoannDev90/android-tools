# uitool

Practical Android UI driver — dump, find, tap, shot over `adb`.

Talk to a device from the shell: read the UI hierarchy as readable lines,
locate nodes by label, act on them (tap, long-press, swipe), and capture
screenshots with crop/scale — without writing a single line of UiAutomator code.

## Requirements

- `adb` (Android SDK platform-tools) in `PATH`
- A connected device/emulator, USB debugging authorized
- Screen **on and unlocked** for `dump`/`find`/`wait`/`tap <label>` (uiautomator
  needs an active window)

## Install

```bash
cargo install --path .        # from this folder
# or, from the repository root:
cargo install --path uitool
```

## Usage

```bash
uitool wake                                  # wake + dismiss keyguard + collapse status bar
uitool dump                                  # text [bounds] * per labeled node
uitool find "Settings"                       # matching nodes (bounds + center), exit 1 if none
uitool find "Settings" --clickable           # clickable matches only
uitool wait "Aperçu" -t 20                  # poll until the node appears
uitool tap 978 2316                          # tap by coordinates
uitool tap "Aperçu"                          # tap by label (center of the node)
uitool long-press "App icon"
uitool swipe 540 2000 540 800 300            # x1 y1 x2 y2 [ms]
uitool shot -o /tmp/screen.png               # screenshot (prints the path)
uitool shot --crop 0,100,1080,400 --scale 2  # crop then nearest upscale
uitool shot --wake --quality 90
uitool launch com.example.myapp              # resolves the launcher activity
uitool launch com.example.myapp/.MainActivity
uitool key HOME                              # or numeric: uitool key 3
uitool input "hello world"                   # spaces → %s for adb
uitool stop com.example.myapp                # force-stop
```

Global flag `-s, --serial <SERIAL>` selects a device; with exactly one device
connected it is picked automatically.

### Subcommands

| Command | Arguments | Description |
|---------|-----------|-------------|
| `wake` | — | Wake + dismiss keyguard + collapse status bar |
| `dump` | — | One line per labeled node: `text [x1,y1,x2,y2]` (`*` = clickable) |
| `find` | `<query> [--clickable]` | Case-insensitive substring on text/content-desc. Prints `[bounds] cx= cy= clickable=`; **exit 1 if no match** |
| `wait` | `<query> [-t/--timeout <secs>]` | Poll the dump until a match (default 10 s), print its bounds |
| `tap` | `<x> <y>` or `<label>` `[-d/--duration <ms>]` | Tap by coordinates, or by label (center of best match). `0` (default) = quick tap, `>0` holds the press |
| `long-press` | `<query> [-d/--duration <ms>]` | Long-press a label (default 600 ms) |
| `swipe` | `<x1> <y1> <x2> <y2> [ms]` | Swipe (default 300 ms) |
| `shot` | `[-o PATH] [--crop x1,y1,x2,y2] [--scale N] [--max-dim N] [--quality 1-100] [--wake]` | Screenshot → jpg/png/webp, prints the output path |
| `launch` | `<pkg>` or `<pkg/.Activity>` | Start an activity; a bare package resolves its launcher activity (`cmd package resolve-activity`) |
| `key` | `<CODE>` | `KEYCODE_HOME` or numeric code |
| `input` | `<text>` | Type text |
| `stop` | `<pkg>` | Force-stop a package |

### Screenshot notes

- Default output: `/tmp/uitool-<epoch>.jpg`; extension picks the encoder
  (`.jpg` with `--quality`, `.png`, `.webp`).
- `--crop` is in screenshot pixels and clamped to the screen; an empty crop is
  an error.
- `--scale` uses nearest-neighbour (good for inspecting icons/density),
  `--max-dim` caps the longest side with a smooth resize.

### Selection notes

`find`/`wait`/`tap <label>` match on `text`, falling back to `content-desc`,
case-insensitively. Tap targets prefer a **clickable** match, otherwise the
first match; the tap lands on the node's bounds center.

## Development

```bash
cargo test          # unit tests (bounds parsing, hierarchy parsing — no device needed)
cargo clippy
cargo build --release
```

Source layout: `src/main.rs` (CLI), `adb.rs` (press, wake, screencap),
`ui.rs` (hierarchy dump + node matching), `shot.rs` (screenshot
post-processing). Depends on [`adbkit`](../adbkit/) for all generic
`adb` plumbing, including launcher-activity resolution.

## License

MIT — see [LICENSE](LICENSE).
