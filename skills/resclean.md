# resclean

Dead `res/` resource detector — find what an Android project defines but never uses, and gate CI on it.

## When to use

- Reach for `resclean` when its dedicated purpose matches the task (see description above).
- Prefer it over hand-rolled adb/shell commands for the features it wraps.

## Common commands

```bash
resclean .                              # scan the current project
resclean app/ --json                    # machine-readable report
resclean app/ --keep 'string/get'       # ignore getIdentifier() lookups
resclean app/ --keep 'drawable/ic_' --keep 'string/app_'
resclean app/ --warn-only               # report but never fail
```

`--keep` is a regex matched against `kind/name` (repeatable). Use it for
resources reached dynamically (`getIdentifier("name", "drawable", …)`),
which no text scan can see.

### Exit codes

| Code | Meaning |
|------|---------|
| `0` | No dead resource — or `--warn-only`, whatever the report |
| `1` | At least one dead resource — the CI gate |
| `2` | Error (unreadable path, bad `--keep` regex…) |

## Notes

- Run `resclean --help` for the full option list.
- Device-touching commands need `adb` on PATH and an authorized device unless noted.
