# android-tools

adb-based tools for Android development, written in Rust.

| Crate | Description | Downloads |
|-------|-------------|-----------|
| [`adbkit`](adbkit/) | Shared adb helpers for the android-tools workspace | ![adbkit](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2FYoannDev90%2Fandroid-tools%2Fmain%2Fstats.json&query=$.tools.adbkit.total&label=adbkit&style=for-the-badge) |
| [`apkctl`](apkctl/) | App lifecycle over adb — clear data/cache, restart, kill, uninstall, deep links, one or every device | ![apkctl](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2FYoannDev90%2Fandroid-tools%2Fmain%2Fstats.json&query=$.tools.apkctl.total&label=apkctl&style=for-the-badge) |
| [`assetgen`](assetgen/) | Android density asset generator — launcher, notification, adaptive, 9-patch, debug badges from one source PNG | ![assetgen](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2FYoannDev90%2Fandroid-tools%2Fmain%2Fstats.json&query=$.tools.assetgen.total&label=assetgen&style=for-the-badge) |
| [`dp2px`](dp2px/) | px ↔ dp ↔ sp converter — offline, or against the device's real density | ![dp2px](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2FYoannDev90%2Fandroid-tools%2Fmain%2Fstats.json&query=$.tools.dp2px.total&label=dp2px&style=for-the-badge) |
| [`iconkit`](iconkit/) | Material Symbols icon search and downloader — browse fonts.google.com/icons, fetch SVG or VectorDrawable | ![iconkit](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2FYoannDev90%2Fandroid-tools%2Fmain%2Fstats.json&query=$.tools.iconkit.total&label=iconkit&style=for-the-badge) |
| [`layoutlint`](layoutlint/) | UI hierarchy lint — touch targets, overlap, labels, from a uiautomator dump | ![layoutlint](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2FYoannDev90%2Fandroid-tools%2Fmain%2Fstats.json&query=$.tools.layoutlint.total&label=layoutlint&style=for-the-badge) |
| [`logstream`](logstream/) | Live Android log streamer - logcat, but actually nice | ![logstream](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2FYoannDev90%2Fandroid-tools%2Fmain%2Fstats.json&query=$.tools.logstream.total&label=logstream&style=for-the-badge) |
| [`perfpoll`](perfpoll/) | Per-app performance sampler over adb — CPU, RAM, battery drain, fps to CSV/JSON | ![perfpoll](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2FYoannDev90%2Fandroid-tools%2Fmain%2Fstats.json&query=$.tools.perfpoll.total&label=perfpoll&style=for-the-badge) |
| [`resclean`](resclean/) | Dead res/ resource detector for Android projects — CI gate | ![resclean](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2FYoannDev90%2Fandroid-tools%2Fmain%2Fstats.json&query=$.tools.resclean.total&label=resclean&style=for-the-badge) |
| [`scenediff`](scenediff/) | Visual regression diff of two screenshots, files or device captures | ![scenediff](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2FYoannDev90%2Fandroid-tools%2Fmain%2Fstats.json&query=$.tools.scenediff.total&label=scenediff&style=for-the-badge) |
| [`startperf`](startperf/) | App cold/warm start benchmark over adb | ![startperf](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2FYoannDev90%2Fandroid-tools%2Fmain%2Fstats.json&query=$.tools.startperf.total&label=startperf&style=for-the-badge) |
| [`svg2vd`](svg2vd/) | SVG → Android VectorDrawable XML converter — pure, no device, no Android SDK | ![svg2vd](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2FYoannDev90%2Fandroid-tools%2Fmain%2Fstats.json&query=$.tools.svg2vd.total&label=svg2vd&style=for-the-badge) |
| [`uitool`](uitool/) | Practical Android UI driver — dump, find, tap, shot over adb | ![uitool](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fraw.githubusercontent.com%2FYoannDev90%2Fandroid-tools%2Fmain%2Fstats.json&query=$.tools.uitool.total&label=uitool&style=for-the-badge) |

## Requirements
- **Rust**: stable (verified with rustc 1.99)
- **adb**: Android SDK platform-tools in PATH
- **Device**: Connected Android device or emulator with USB debugging authorized

## Install

Binaries for Linux (musl, any distro), Windows and macOS are attached to
each [release](https://github.com/YoannDev90/android-tools/releases). Pick
a tool, then:

```bash
# Linux / macOS — detects OS and arch, installs to ~/.local/bin:
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/YoannDev90/android-tools/releases/latest/download/apkctl-installer.sh | sh

# Windows (PowerShell):
irm https://github.com/YoannDev90/android-tools/releases/latest/download/apkctl-installer.ps1 | iex

# macOS / Linuxbrew:
brew install YoannDev90/homebrew-tap/apkctl

# Debian / Ubuntu (.deb) and Fedora / RHEL (.rpm) attached to the release:
sudo apt install ./apkctl_*_amd64.deb

# From crates.io (needs Rust):
cargo install apkctl

# Windows package manager:
winget install YoannDev90.apkctl
```

Replace `apkctl` with any tool from the table above. See
[`RELEASING.md`](RELEASING.md) for the full release and packaging chain.

## Build
```bash
cargo build --release
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

## Code generation

`index.json`, this README and `stats.json` are generated — edit the
sources, then:

```bash
python3 scripts/generate_index.py   # .METADATA.toml -> index.json
python3 scripts/generate_readme.py  # index.json -> README.md
python3 scripts/generate_stats.py   # GitHub + crates.io -> stats.json
```

`.METADATA.toml` lives at the root of each tool directory and holds the
tool description, tags, requirements and dependency metadata.

CI (`.github/workflows/ci.yml`) runs fmt/clippy/test and fails when these
generated files are out of date with their sources. Only
`generate_stats.py` reaches the network, so it runs on its own schedule
(`.github/workflows/stats.yml`) instead of in CI.

## Repository layout

```
.
├── Cargo.toml
├── Cargo.lock
├── index.json
├── stats.json
├── README.md
├── LICENSE
├── rustfmt.toml
├── .github/
│   ├── dependabot.yml
│   └── workflows
├── apkctl/
│   ├── .METADATA.toml
│   ├── Cargo.toml
│   ├── LICENSE
│   ├── README.md
│   └── src
├── assetgen/
│   ├── .METADATA.toml
│   ├── Cargo.toml
│   ├── LICENSE
│   ├── README.md
│   └── src
├── dp2px/
│   ├── .METADATA.toml
│   ├── Cargo.toml
│   ├── LICENSE
│   ├── README.md
│   └── src
├── iconkit/
│   ├── .METADATA.toml
│   ├── Cargo.toml
│   ├── LICENSE
│   ├── README.md
│   └── src
├── layoutlint/
│   ├── .METADATA.toml
│   ├── Cargo.toml
│   ├── LICENSE
│   ├── README.md
│   └── src
├── perfpoll/
│   ├── .METADATA.toml
│   ├── Cargo.toml
│   ├── LICENSE
│   ├── README.md
│   └── src
├── resclean/
│   ├── .METADATA.toml
│   ├── Cargo.toml
│   ├── LICENSE
│   ├── README.md
│   ├── src
│   └── tests
├── scenediff/
│   ├── .METADATA.toml
│   ├── Cargo.toml
│   ├── LICENSE
│   ├── README.md
│   └── src
├── svg2vd/
│   ├── .METADATA.toml
│   ├── Cargo.toml
│   ├── LICENSE
│   ├── README.md
│   └── src
├── adbkit/
│   ├── .METADATA.toml
│   ├── Cargo.toml
│   ├── LICENSE
│   ├── README.md
│   ├── src
│   └── tests
├── logstream/
│   ├── .METADATA.toml
│   ├── Cargo.toml
│   ├── LICENSE
│   ├── README.md
│   └── src
├── startperf/
│   ├── .METADATA.toml
│   ├── Cargo.toml
│   ├── LICENSE
│   ├── README.md
│   └── src
├── uitool/
│   ├── .METADATA.toml
│   ├── Cargo.toml
│   ├── LICENSE
│   ├── README.md
│   └── src
└── scripts/
    ├── __pycache__
    ├── generate_index.py
    ├── generate_readme.py
    ├── generate_stats.py
    └── make_nfpm.py
```

## License

MIT — see [LICENSE](LICENSE). Each crate folder carries its own copy.
