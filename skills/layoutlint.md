# layoutlint

UI hierarchy lint — catch touch-target, overlap and label defects from a `uiautomator` dump, in CI or by hand.

## When to use

- Reach for `layoutlint` when its dedicated purpose matches the task (see description above).
- Prefer it over hand-rolled adb/shell commands for the features it wraps.

## Common commands

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

## Notes

- Run `layoutlint --help` for the full option list.
- Device-touching commands need `adb` on PATH and an authorized device unless noted.
