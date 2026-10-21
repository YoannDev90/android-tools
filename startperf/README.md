# startperf

App cold/warm start benchmark over `adb` — measure launch times without
Android Studio.

Runs `am start -W` N times against an activity, parses the timings the
framework reports, and summarizes them (min / median / mean / max / stddev).
Useful to compare two builds, two devices, or a change before/after.

## Requirements

- `adb` (Android SDK platform-tools) in `PATH`
- A connected device/emulator, USB debugging authorized
- The target app installed

## Install

```bash
cargo install --path .        # from this folder
# or, from the repository root:
cargo install --path startperf
```

## Usage

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

## Output

```text
Cold start — com.example.myapp/.MainActivity

 run    total ms     wait ms     this ms    launch
------------------------------------------------------
   1         842         852           -      COLD
   2         900           -         880      WARM
------------------------------------------------------
total   n=2  min 842  median 871.0  mean 871.0  max 900  ms  (stddev 41.0)
```

- **total** — `TotalTime`, the number to compare: time to the first drawn frame.
- **wait** — `WaitTime`, until the launch is fully complete (later windows).
- **this** — `ThisTime`, top activity only (not reported by every release).
- **launch** — the state Android itself reported (`COLD`/`WARM`/`HOT`), a
  cross-check on the mode you asked for.

`--json` prints the same report as an object (`mode`, `package`, `component`,
`requested_runs`, `skipped`, `stats`, `runs[]`). `--csv` writes one row per
measured run with a header, so it opens directly in a spreadsheet; the table
still goes to stdout.

### Skipped runs

`am start -W` prefixes a `Warning:` line when it did not do a fresh start, and
the wording varies by release. Two shapes matter:

- *"Activity not started, its current task has been brought to the front"* —
  the task was resumed. Skipped in **cold** mode only: the `force-stop` had no
  effect, so it would measure a resume. In **warm** mode this is the expected
  answer after HOME (`LaunchState: HOT`, real `TotalTime`), so it counts.
- *"Activity not started, intent has been delivered to currently running
  top-most instance"* (Android 13+) — the activity was already on top and
  Android launched nothing, reporting `TotalTime: 0`. Skipped in **every**
  mode: counting it would put a fake 0 ms in the stats.

Skipped runs are listed separately and excluded from `n` and the stats.

## Method notes

- Settling pauses (`--settle`, default 500 ms) after the force-stop and after
  each run, so the device is not still busy when the next measurement starts.
- Stats are computed over successful runs only; `n` in the summary is the
  number of samples, not the number of requested runs.
- Median is reported alongside the mean because cold-start samples are usually
  skewed by one GC/JIT hiccup, which the mean absorbs.
- The **launch** column is ground truth, not the mode you asked for. A `warm`
  run reported as `COLD` means the process was reclaimed while backgrounded
  (low memory) — the app genuinely cold-started, so treat that sample with the
  cold numbers in mind.

## Development

```bash
cargo test          # 32 unit tests — parser, stats, loop and CSV/JSON, no device needed
cargo clippy
cargo build --release
```

Source layout: `main.rs` (CLI), `parse.rs` (`am start -W` output), `run.rs`
(measurement loop behind a `Device` trait, so the cold/warm decisions are
tested with a fake), `stats.rs` (summary), `output.rs` (table/JSON/CSV).
Depends on [`adbkit`](../adbkit/) for all generic `adb` plumbing, including
launcher-activity resolution.

## License

MIT — see [LICENSE](LICENSE).
