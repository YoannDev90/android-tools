---
name: logstream
description: Live Android log streamer — `logcat`, but actually nice.
---

# logstream

Live Android log streamer — `logcat`, but actually nice.

## When to use

- Reach for `logstream` when its dedicated purpose matches the task (see description above).
- Prefer it over hand-rolled adb/shell commands for the features it wraps.

## Common commands

```bash
logstream                                   # everything (all processes)
logstream -p com.example.myapp              # follow one app (pid-filtered)
logstream -p com.example.myapp -l w         # warnings and above
logstream -p com.example.myapp -t OkHttp -t Retrofit
logstream -p com.example.myapp --skip Choreographer
logstream -p com.example.myapp --json | jq .
logstream -p com.example.myapp --plain -o run.log
logstream -p com.example.myapp --clear      # drop the buffer first
```

Package resolution tries `<pkg>` then `<pkg>.debug`, so release/debug builds
work with the same flag. With several devices attached, pass `-s <serial>`.

### Options

| Flag | Default | Description |
|------|---------|-------------|
| `-p, --package <PKG>` | — (all processes) | App package name (`.debug` fallback auto-tried) |
| `-s, --serial <SERIAL>` | auto | adb serial; auto-picked when a single device is connected |
| `--all` | off | Show every process (disable app-pid filtering) |
| `-l, --level <v\|d\|i\|w\|e\|f>` | `v` | Minimum level to display |
| `-t, --tag <PREFIX>` | — | Keep only tags starting with this prefix (repeatable) |
| `--skip <PREFIX>` | — | Drop tags starting with this prefix (repeatable) |
| `-q, --search <QUERY>` | empty | Initial search (regex); also filterable live with `/` |
| `-b, --buffer <LINES>` | `10000` | Ring-buffer capacity |
| `--clear` | off | Clear logcat buffer before streaming |
| `--json` | off | One JSON object per record (JSONL) |
| `--plain` | off | Plain colored output, no TUI (auto when stdout is not a TTY) |
| `-o, --out <PATH>` | — | Mirror everything shown (raw records + `[logstream]` banner lines), any mode |

### Modes

- **TUI** (default on a TTY): interactive view, mouse scroll supported.
- **`--plain`**: colored line output for terminals/pipes.
- **`--json`**: JSONL, one record per line (`epoch_ms`, `time`, `pid`, `tid`, `level`, `tag`, `msg`) — for `jq`, `grep`, log shippers.
- Any mode can mirror to a file with `-o`.

### TUI keys

| Key | Action |
|-----|--------|
| `q` / `Ctrl+C` | Quit |
| `/` | Search (live regex filter) |
| `n` / `N` | Next / previous match |
| `+` / `-` | Add tag to allow-list / deny-list |
| `:` | Set minimum level (`v d i w e f`) |
| `c` | Clear buffer |
| `Ctrl+A` | Toggle app-pid filter vs all pids |
| `PgUp` / `PgDn` | Scroll view by 10 |
| `Home` / `End` | Top / follow live (unfreeze) |
| `↑` / `↓`, mouse wheel | Scroll by 1 / 3 |

## Notes

- Run `logstream --help` for the full option list.
- Device-touching commands need `adb` on PATH and an authorized device unless noted.
