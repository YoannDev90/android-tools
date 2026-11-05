---
name: startperf
description: App cold/warm start benchmark over `adb` — measure launch times without Android Studio.
---

# startperf

App cold/warm start benchmark over `adb` — measure launch times without Android Studio.

## When to use

- Reach for `startperf` when its dedicated purpose matches the task (see description above).
- Prefer it over hand-rolled adb/shell commands for the features it wraps.

## Common commands

```bash
startperf -p com.example.myapp                  # 5 cold runs (default)
startperf -p com.example.myapp --runs 10        # more samples
startperf -p com.example.myapp --warm           # HOME between runs: warm start
startperf -p com.example.myapp --mode warm      # same, explicit
startperf -p com.example.myapp --json           # machine-readable report
startperf -p com.example.myapp --csv runs.csv   # samples to a spreadsheet
startperf -n com.example.myapp/.MainActivity    # skip launcher resolution
startperf -p com.example.myapp --settle 1000    # slower cadence, calmer device
```

Global flag `-s, --serial <SERIAL>` selects a device; with exactly one device
connected it is picked automatically.

### Cold vs warm

| Mode | What happens before each run | What it measures |
|------|------------------------------|------------------|
| `cold` (default) | `am force-stop <pkg>`, then a pause | Full init: process creation, `Application` init, first frame |
| `warm` | `KEYCODE_HOME`, then a pause | Activity recreated against a resident process |

`warm` presses HOME on purpose. Relaunching an app that is still top-most makes
Android deliver the intent without starting anything (`TotalTime: 0`), which is
not a warm start — just the app already being in front.

## Notes

- Run `startperf --help` for the full option list.
- Device-touching commands need `adb` on PATH and an authorized device unless noted.
