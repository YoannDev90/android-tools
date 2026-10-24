# android-tools

adb-based tools for Android development, written in Rust.

| Crate | Description |
|-------|-------------|
| [`adbkit`](adbkit/) | Shared adb helpers for the android-tools workspace |
| [`apkctl`](apkctl/) | App lifecycle over adb — clear data/cache, restart, kill, uninstall, deep links, one or every device |
| [`assetgen`](assetgen/) | Android density asset generator — launcher, notification, adaptive, 9-patch, debug badges from one source PNG |
| [`dp2px`](dp2px/) | px ↔ dp ↔ sp converter — offline, or against the device's real density |
| [`iconkit`](iconkit/) | Material Symbols icon search and downloader — browse fonts.google.com/icons, fetch SVG or VectorDrawable |
| [`layoutlint`](layoutlint/) | UI hierarchy lint — touch targets, overlap, labels, from a uiautomator dump |
| [`logstream`](logstream/) | Live Android log streamer - logcat, but actually nice |
| [`perfpoll`](perfpoll/) | Per-app performance sampler over adb — CPU, RAM, battery drain, fps to CSV/JSON |
| [`resclean`](resclean/) | Dead res/ resource detector for Android projects — CI gate |
| [`scenediff`](scenediff/) | Visual regression diff of two screenshots, files or device captures |
| [`startperf`](startperf/) | App cold/warm start benchmark over adb |
| [`svg2vd`](svg2vd/) | SVG → Android VectorDrawable XML converter — pure, no device, no Android SDK |
| [`uitool`](uitool/) | Practical Android UI driver — dump, find, tap, shot over adb |

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

`index.json` and this README are generated — edit the sources, then:

```bash
python3 scripts/generate_index.py   # .METADATA.toml -> index.json
python3 scripts/generate_readme.py  # index.json -> README.md
```

`.METADATA.toml` lives at the root of each tool directory and holds the
tool description, tags, requirements and dependency metadata.

CI (`.github/workflows/ci.yml`) runs fmt/clippy/test and fails when these
generated files are out of date with their sources.

## Repository layout

```
.
├── Cargo.toml
├── Cargo.lock
├── index.json
├── README.md
├── LICENSE
├── rustfmt.toml
├── .github/
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
    ├── generate_index.py
    ├── generate_readme.py
    └── make_nfpm.py
```

## License

MIT — see [LICENSE](LICENSE). Each crate folder carries its own copy.
