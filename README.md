# android-tools

adb-based tools for Android development, written in Rust.

## Quick install

```bash
# Linux / macOS — pick the tools in a selector, installs to ~/.local/bin:
curl -LsSf https://raw.githubusercontent.com/YoannDev90/android-tools/main/scripts/install.sh | bash
```

```powershell
# Windows — same selector, installs to %LOCALAPPDATA%\android-tools\bin:
irm https://raw.githubusercontent.com/YoannDev90/android-tools/main/scripts/install.ps1 | iex
```

The same scripts are the updater for what they installed: append
`| bash -s -- update`, and see [Install](#install) for `list`,
`uninstall`, `--all` / `--bin-dir` and every other channel.

<!-- BEGIN:tools-table -->
| Crate | Description | Downloads |
|-------|-------------|-----------|
| [`adbkit`](src/adbkit/) | Shared adb helpers for the android-tools workspace | ![adbkit](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2FYoannDev90%2Fandroid-tools%2Fmain%2Fstats.json&query=$.tools.adbkit.total&label=adbkit&style=for-the-badge) |
| [`apkctl`](src/apkctl/) | App lifecycle over adb — clear data/cache, restart, kill, uninstall, deep links, one or every device | ![apkctl](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2FYoannDev90%2Fandroid-tools%2Fmain%2Fstats.json&query=$.tools.apkctl.total&label=apkctl&style=for-the-badge) |
| [`assetgen`](src/assetgen/) | Android density asset generator — launcher, notification, adaptive, 9-patch, debug badges from one source PNG | ![assetgen](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2FYoannDev90%2Fandroid-tools%2Fmain%2Fstats.json&query=$.tools.assetgen.total&label=assetgen&style=for-the-badge) |
| [`dp2px`](src/dp2px/) | px ↔ dp ↔ sp converter — offline, or against the device's real density | ![dp2px](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2FYoannDev90%2Fandroid-tools%2Fmain%2Fstats.json&query=$.tools.dp2px.total&label=dp2px&style=for-the-badge) |
| [`iconkit`](src/iconkit/) | Material Symbols icon search and downloader — browse fonts.google.com/icons, fetch SVG or VectorDrawable | ![iconkit](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2FYoannDev90%2Fandroid-tools%2Fmain%2Fstats.json&query=$.tools.iconkit.total&label=iconkit&style=for-the-badge) |
| [`layoutlint`](src/layoutlint/) | UI hierarchy lint — touch targets, overlap, labels, from a uiautomator dump | ![layoutlint](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2FYoannDev90%2Fandroid-tools%2Fmain%2Fstats.json&query=$.tools.layoutlint.total&label=layoutlint&style=for-the-badge) |
| [`logstream`](src/logstream/) | Live Android log streamer - logcat, but actually nice | ![logstream](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2FYoannDev90%2Fandroid-tools%2Fmain%2Fstats.json&query=$.tools.logstream.total&label=logstream&style=for-the-badge) |
| [`perfpoll`](src/perfpoll/) | Per-app performance sampler over adb — CPU, RAM, battery drain, fps to CSV/JSON | ![perfpoll](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2FYoannDev90%2Fandroid-tools%2Fmain%2Fstats.json&query=$.tools.perfpoll.total&label=perfpoll&style=for-the-badge) |
| [`resclean`](src/resclean/) | Dead res/ resource detector for Android projects — CI gate | ![resclean](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2FYoannDev90%2Fandroid-tools%2Fmain%2Fstats.json&query=$.tools.resclean.total&label=resclean&style=for-the-badge) |
| [`scenediff`](src/scenediff/) | Visual regression diff of two screenshots, files or device captures | ![scenediff](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2FYoannDev90%2Fandroid-tools%2Fmain%2Fstats.json&query=$.tools.scenediff.total&label=scenediff&style=for-the-badge) |
| [`startperf`](src/startperf/) | App cold/warm start benchmark over adb | ![startperf](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2FYoannDev90%2Fandroid-tools%2Fmain%2Fstats.json&query=$.tools.startperf.total&label=startperf&style=for-the-badge) |
| [`svg2vd`](src/svg2vd/) | SVG → Android VectorDrawable XML converter — pure, no device, no Android SDK | ![svg2vd](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2FYoannDev90%2Fandroid-tools%2Fmain%2Fstats.json&query=$.tools.svg2vd.total&label=svg2vd&style=for-the-badge) |
| [`uitool`](src/uitool/) | Practical Android UI driver — dump, find, tap, shot over adb | ![uitool](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2FYoannDev90%2Fandroid-tools%2Fmain%2Fstats.json&query=$.tools.uitool.total&label=uitool&style=for-the-badge) |
<!-- END:tools-table -->

## Requirements

- **Rust**: stable (verified with rustc 1.99)
- **adb**: Android SDK platform-tools in PATH
- **Device**: Connected Android device or emulator with USB debugging authorized

## Install

See the per-OS guides:

- [Linux](docs/install-linux.md)
- [macOS](docs/install-macos.md)
- [Windows](docs/install-windows.md)

Replace `apkctl` with any tool from the table above. See
[`RELEASING.md`](RELEASING.md) for the full release and packaging chain.

## Build

```bash
cargo build --release
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```


## License

MIT — see [LICENSE](LICENSE). Each crate folder carries its own copy.
