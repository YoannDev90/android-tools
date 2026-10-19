# logstream

Live Android log streamer — `logcat`, but actually nice.

Streams `adb logcat` for **one app package** (or every process, when no
package is given) and gives you what stock `logcat` doesn't: a real TUI with
search, level/tag filters, pid tracking, and banners for app start / restart /
crash — plus plain and JSONL modes for scripts.

## Requirements

- `adb` (Android SDK platform-tools) in `PATH`
- A connected device/emulator, USB debugging authorized
- With `-p`: the target package installed on the device

## Install

```bash
cargo install --path .        # from this folder
# or, from the repository root:
cargo install --path logstream
```

## Usage

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

## Behavior notes

- Logcat is read with `-v threadtime -b main,system,crash -T 1`: history is
  skipped, crash buffer included, newest line first.
- Stack-trace continuation lines are merged into the record that started them.
- The app pid is polled continuously: `Started` / `Restarted` / `Died` banners
  appear on change, and pids of dead processes are kept so crash history
  survives an app restart.
- `FATAL EXCEPTION` on tag `AndroidRuntime` triggers a crash banner.
- If `adb logcat` exits (device unplugged), you get a notice instead of a hang.

## Development

```bash
cargo test          # unit tests (parse, buffer, render, TUI — no device needed)
cargo clippy
cargo build --release
```

Source layout: `src/main.rs` (CLI/modes), `adb.rs` (logcat + pid watcher +
package resolution), `parse.rs` (threadtime parser), `buffer.rs` (ring
buffer/filters), `render.rs` (plain/JSONL renderers), `tui.rs` (ratatui app).
Depends on [`adbkit`](../adbkit/) for all generic `adb` plumbing.

## License

MIT — see [LICENSE](LICENSE).
