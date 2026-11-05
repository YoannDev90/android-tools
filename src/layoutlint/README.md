# layoutlint

UI hierarchy lint — catch touch-target, overlap and label defects from a
`uiautomator` dump, in CI or by hand.

Reads the same hierarchy `uitool` acts on (via `adbkit`), applies four
rules and reports findings as text or JSON. CI can gate on the exit
code; `--xml` replays a saved dump with no device attached.

## Requirements

- None for `--xml` replay (CI needs no device)
- `adb` (Android SDK platform-tools) in `PATH` and a connected device
  only for `--device`

## Install

```bash
cargo install --path .        # from this folder
# or, from the repository root:
cargo install --path layoutlint
```

## Usage

```bash
layoutlint --device                          # dump the device, lint it
layoutlint --xml dump.xml --density 440      # replay a saved dump (CI)
layoutlint --xml dump.xml --json             # machine-readable report
layoutlint --xml dump.xml --min-touch 44     # stricter than 48 dp
layoutlint --device --warn-only              # report but never fail
```

Dump a screen first with `uitool dump -o dump.xml`, or run
`layoutlint --device` directly. `-s, --serial` selects a device; with
exactly one connected it is picked automatically.

### Exit codes

| Code | Meaning |
|------|---------|
| `0` | No finding — or `--warn-only`, whatever the findings |
| `1` | At least one finding (any severity) — the CI gate |
| `2` | Error (unreadable dump, no device, screen off…) |

Severity orders the report; it does not decide the gate: a warning-only
run still exits 1 unless you pass `--warn-only`.

## Rules

| Rule | What it catches | Severity |
|------|-----------------|----------|
| `touch-size` | Tap target smaller than `--min-touch` dp (default 48) in either dimension | warning |
| `overlap` | Two tap targets whose bounds intersect by more than 20% of the smaller one | error |
| `unlabeled` | Clickable node with neither `text` nor `content-desc` (id-only, icon-only) | error |
| `duplicate-label` | Label displayed on a non-clickable node while an identical clickable target exists | warning |

```text
3 nodes linted at 48 dp
warning  touch-size       target "Send" is 36.4x33.5 dp (min 48)  [800,1800][900,1892]
1 finding (0 errors, 1 warning)
```

`--json` reports `nodes`, `density`, `min_touch`, `findings[]`
(`severity`, `rule`, `message`, `bounds`), `errors`, `warnings`,
`failed`.

## Method notes

- px→dp uses the screen density: `--device` reads `wm density` (the
  override wins when set — that is what apps draw at), 160 dpi is the
  default for `--xml`. `--density` overrides both.
- The touch threshold is rounded **up**, so a target exactly at the
  guidance is never flagged by a truncation.
- The node set is `adbkit::hierarchy::parse_hierarchy`: nodes with a
  label or an id. A clickable row whose label sits on its children (the
  standard Android pattern) is not an `unlabeled` finding — the rule
  hunts icon-only views, not containers.
- `overlap` is strict: exactly 20% passes, targets that merely touch
  (zero-area intersection) are not stacked.
- `duplicate-label` compares only what renders (`text`, `content-desc`);
  resource-ids are never displayed, so they are never matched.

## Development

```bash
cargo test          # 15 unit tests — 4 rules + fixtures, density parsing, reports; no device needed
cargo clippy
cargo build --release
```

Source layout: `main.rs` (CLI and wiring), `rules.rs` (the four rules and
their reports), `device.rs` (`uiautomator dump`, `wm density`). Depends
on [`adbkit`](../adbkit/) for the hierarchy model and adb access.

## License

MIT — see [LICENSE](LICENSE).
