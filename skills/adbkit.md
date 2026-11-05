# adbkit

Shared `adb` helpers for the android-tools workspace. Generic plumbing only: device picking, shell/exec-out wrappers and launcher-activity resolution. Domain logic (logcat streaming, UI hierarchy, screenshots) stays in the tools that own it.

## When to use

- Reach for `adbkit` when its dedicated purpose matches the task (see description above).
- Prefer it over hand-rolled adb/shell commands for the features it wraps.

## Common commands

See `src/adbkit/README.md` for usage.

## Notes

- Run `adbkit --help` for the full option list.
- Device-touching commands need `adb` on PATH and an authorized device unless noted.
