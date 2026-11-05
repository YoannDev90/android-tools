# perfpoll

Per-app performance sampler over `adb` — CPU, RAM, battery drain and frame rate, on a fixed interval, to CSV or JSON.

## When to use

- Reach for `perfpoll` when its dedicated purpose matches the task (see description above).
- Prefer it over hand-rolled adb/shell commands for the features it wraps.

## Common commands

```bash
perfpoll -p com.example.app                      # 30 s of 1 s ticks, CSV to stdout
perfpoll -p com.foo -p com.bar -i 500 -d 60      # two apps, 500 ms interval
perfpoll -p com.foo -o perf.csv                  # CSV to a file, summary to stdout
perfpoll -p com.foo --json -o perf.json          # one JSON document at the end
perfpoll -p com.foo -d 0                         # run until Ctrl+C
```

| Flag | Default | Meaning |
|------|---------|---------|
| `-p, --package` | — | package to sample, repeatable |
| `-i, --interval-ms` | `1000` | sampling interval |
| `-d, --duration-s` | `30` | run length; `0` = until interrupted |
| `-o, --out` | stdout | CSV/JSON destination |
| `--json` | off | one JSON document instead of streamed CSV |
| `--serial` | auto | adb serial (auto-picked with a single device) |

### Exit codes

| Code | Meaning |
|------|---------|
| `0` | Run completed |
| `2` | Error (no device, package not installed, `adb` failure…) |

## Notes

- Run `perfpoll --help` for the full option list.
- Device-touching commands need `adb` on PATH and an authorized device unless noted.
