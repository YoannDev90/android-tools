# adbkit

Shared `adb` helpers for the android-tools workspace. Generic plumbing only:
device picking, shell/exec-out wrappers and launcher-activity resolution.
Domain logic (logcat streaming, UI hierarchy, screenshots) stays in the tools
that own it.

## API

- `pick_serial(explicit)` — explicit flag, or the single connected device
- `parse_devices(stdout)` — device ids in state `device` (pure, testable)
- `resolve_launcher(dev, package)` — the package's launcher activity
  (`cmd package resolve-activity --brief`)
- `parse_component(stdout)` — last `package/class` line of such a dump
  (pure, testable)
- `device_density(dev)` — the density the UI draws at, in dpi
  (`wm density`, override wins over physical)
- `parse_wm_density(stdout)` — the same, parsed (pure, testable)
- `Adb { serial }` — per-device handle:
  - `shell(args)` — fails on non-zero exit
  - `shell_opt(args)` — ignores non-zero exit (`pm path`, `pidof` failures are normal)
  - `shell_timeout(args, timeout)` — hard kill on hang
  - `shell_quiet(args)` — ignores everything
  - `exec_out(args)` — raw stdout bytes (screencap, cat)
- `ChildHandle` — shared handle to kill a spawned child

## Development

```bash
cargo test -p adbkit
```

## License

MIT — see [LICENSE](LICENSE).
