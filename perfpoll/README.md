# perfpoll

Per-app performance sampler over `adb` — CPU, RAM, battery drain and frame
rate, on a fixed interval, to CSV or JSON.

One row per app per tick: CPU share, PSS, battery level, per-app mAh,
rendered/janky frames and FPS. Rows stream to CSV as they are taken, so an
interrupted run still keeps its data; `--json` instead emits one document
with per-app summaries at the end.

## Requirements

- `adb` on `PATH`, one connected device (or `--serial` / `ANDROID_SERIAL`).
- The sampled package(s) must be installed; they do not need to be in the
  foreground — a tick where the app is not running is skipped and counted,
  never reported as zeroes.

## Install

```bash
cargo install --path .        # from this folder
# or, from the repository root:
cargo install --path perfpoll
```

## Usage

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

## Output

```text
t_ms,app,cpu_pct,pss_kb,battery_pct,mah_cum,mah_delta,frames,janky,fps
0,com.miui.home,,174251,97,0.1,,,,
1025,com.miui.home,2.3,175180,97,0.1,0,6,0,6.1
2025,com.miui.home,1.5,175112,97,0.1,0,4,0,3.9
```

An empty cell means *not available for that tick* (first tick has no
interval yet, a counter reset, app briefly not running) — it is never a
fake zero. After the CSV, one summary line per app goes to stdout when
`--out` was given, and to stderr otherwise so the CSV stream stays clean:

```text
com.miui.home: 4 samples, 0 skipped | cpu avg 1.7% | pss max 175180 kB | drained 0.00 mAh | fps avg 4.5 | jank 1.2% (1/84)
```

`--json` writes `serial`, `interval_ms`, `duration_s`, `elapsed_s`, every
`sample`, and a `summary` per app (samples, skipped ticks, average CPU,
max PSS, drained mAh, frame totals, average FPS, jank %).

## Method notes

- **CPU%** — `/proc/stat`'s aggregate `cpu` line and each of the app's
  `/proc/<pid>/stat` (`utime + stime`) are read every tick; the delta
  ratio, clamped to 0..=100, is the app's share of total CPU capacity
  across all cores.
- **RAM** — the sum of one `TOTAL` row per process block of
  `dumpsys meminfo <pkg>` (PSS, kB); summary tables never match.
- **Battery** — the global `level:` plus the app's cumulative mAh under
  `Estimated power use (mAh):` of `dumpsys batterystats`, matched on
  package name, uid string (`u0a383`) or bare uid (`UID 1000:` —
  Android 15 prints `UID` uppercase). `mah_delta` is the per-tick
  difference; a backwards step (charge counter reset) rebases to 0.
- **FPS** — `dumpsys gfxinfo` `Total frames rendered` / `Janky frames`
  deltas divided by the *actual* time between ticks (overrunning ticks
  reschedule instead of bursting, so rates never inflate).
- Pid discovery is `pidof`; a pid dying mid-tick simply drops out of the
  CPU sum instead of failing the run.
- First tick reports no rates — absolute counters would poison both the
  series and the summary.

## Development

```bash
cargo test          # 13 unit tests — every parser against real dumpsys /
                    # proc output shapes, delta/clamp math, CSV cells and
                    # summary aggregation; no device or Android SDK needed
cargo clippy
cargo build --release
```

Source layout: `main.rs` (CLI and the sampling loop), `parse.rs` (parsers
and counter math + tests), `output.rs` (rows, summaries, CSV + tests).
The sampling loop itself needs a live device: a smoke run like
`perfpoll -p com.android.systemui -d 5 -o /tmp/perf.csv` is the
device-side check.

## License

MIT — see [LICENSE](LICENSE).
