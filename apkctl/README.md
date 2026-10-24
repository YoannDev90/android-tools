# apkctl

App lifecycle over adb — clear data, restart, kill, uninstall, open
deep links — on one device or every connected one.

The actions IntelliJ's [adb-idea](https://github.com/pbreault/adb-idea)
plugin puts behind a click, as a CLI: `apkctl kill com.foo` between two
tests, `apkctl --all clear-data com.foo` before a demo across the whole
device pool, `apkctl open myapp://detail/7` to poke a deep link. Every
action is one `adb shell` command — nothing gets installed on the
device.

## Requirements

- `adb` (Android SDK platform-tools) in `PATH`
- A connected device/emulator with USB debugging authorized

## Install

```bash
cargo install --path .        # from this folder
# or, from the repository root:
cargo install --path apkctl
```

## Usage

```bash
apkctl kill com.example.app               # force-stop (not running = OK)
apkctl clear-data com.example.app         # pm clear: data + cache
apkctl clear-cache com.example.app        # pm trim-caches 0
apkctl restart com.example.app            # force-stop + relaunch, launcher resolved
apkctl uninstall com.example.app          # pm uninstall
apkctl settings com.example.app           # open App info in Settings
apkctl open 'myapp://detail/7'            # am start -a VIEW -d

apkctl --all kill com.example.app         # every connected device
apkctl -s SERIAL1 -s SERIAL2 restart com.example.app
```

| Subcommand | Device command | Notes |
|------------|----------------|-------|
| `clear-data <pkg>` | `pm clear <pkg>` | wipes data **and** cache |
| `clear-cache <pkg>` | `pm trim-caches 0 <pkg>` | the Storage "clear cached data" path |
| `kill <pkg>` | `am force-stop <pkg>` | not running is a success |
| `restart <pkg>` | `force-stop` + `am start -n <launcher>` | launcher resolved at run time |
| `uninstall <pkg>` | `pm uninstall <pkg>` | permanent, no `-k`/`--user` |
| `settings <pkg>` | `am start -a APPLICATION_DETAILS_SETTINGS -d package:<pkg>` | App info page |
| `open <uri>` | `am start -a VIEW -d '<uri>'` | URI single-quoted for the device shell |

Output is one line per device; with several devices each line is
prefixed `[serial]`. A device that fails gets its error line on stderr
and the run still continues on the others — the exit code is 1 if **any**
device failed, 0 only when all succeeded.

### Notes

- Package names and URIs are **validated, never interpolated**: `;`,
  spaces, quotes and a leading `-` are rejected before anything reaches
  the device shell (adb joins `adb shell` argv with spaces, unescaped).
- `restart` resolves the launcher activity at run time
  (`cmd package resolve-activity`) — it is whatever the installed build
  declares, not a hardcoded `.MainActivity`.
- `kill` on an app that is not running succeeds: `am force-stop` is
  idempotent, which is what makes it usable as a test fixture teardown.
- `clear-cache` goes through `pm trim-caches 0 <pkg>` — the same
  PackageManager cache deletion Settings → Storage → "clear cached data"
  does (SD Maid makes the same call, through Shizuku). The
  `pm clear --cache-only` flag stalls outright on some builds (seen on
  HyperOS/MIUI: no dialog appears, focus never leaves the launcher), so
  it is not used.
- Every device command has a 10s deadline: a stalled `adb shell`
  becomes an error on that device, never a CLI that never returns.
- `settings` opens the App info page — the manual route to the cache
  button, and what accessibility automation drives on-device.

## Development

```bash
cargo test          # pure command building — no device needed
cargo clippy
cargo build --release
```

Source layout: `main.rs` (CLI, per-device fan-out and exit code),
`cmds.rs` (pure command construction and output lines). Depends on
[`adbkit`](../adbkit/) for device listing, launcher resolution and the
`adb shell` wrappers.

## License

MIT — see [LICENSE](LICENSE).
