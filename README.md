# android-tools

adb-based tools for Android development, written in Rust.

| Crate | Description |
|-------|-------------|
| [`adbkit`](adbkit/) | Shared adb helpers for the android-tools workspace |
| [`dpx`](dpx/) | px ↔ dp ↔ sp converter — offline, or against the device's real density |
| [`layoutlint`](layoutlint/) | UI hierarchy lint — touch targets, overlap, labels, from a uiautomator dump |
| [`logstream`](logstream/) | Live Android log streamer - logcat, but actually nice |
| [`resclean`](resclean/) | Dead res/ resource detector for Android projects — CI gate |
| [`scenediff`](scenediff/) | Visual regression diff of two screenshots, files or device captures |
| [`startperf`](startperf/) | App cold/warm start benchmark over adb |
| [`uitool`](uitool/) | Practical Android UI driver — dump, find, tap, shot over adb |

## Requirements
- **Rust**: stable (verified with rustc 1.99)
- **adb**: Android SDK platform-tools in PATH
- **Device**: Connected Android device or emulator with USB debugging authorized

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
├── dpx/
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
    └── generate_readme.py
```

## License

MIT — see [LICENSE](LICENSE). Each crate folder carries its own copy.
