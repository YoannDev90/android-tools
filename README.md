# Android Tools

Two small, sharp CLI tools for Android development over `adb`, written in Rust,
plus the shared library they build on. Each tool is a standalone crate; the
repository is a Cargo workspace.

| Crate | Description |
|-------|-------------|
| [`adbkit`](adbkit/) | Shared `adb` helpers — device picking, shell/exec-out wrappers with timeouts (library) |
| [`logstream`](logstream/) | Live Android log streamer — `logcat`, but actually nice: TUI, app-pid filter, search, crash/restart banners, JSONL output |
| [`uitool`](uitool/) | Practical Android UI driver — dump, find, wait, tap, swipe, screenshot, launch, type over `adb` |

## Requirements

- **Rust** (stable) — install via [rustup.rs](https://rustup.rs). Verified with `rustc 1.99`.
- **adb** (Android SDK platform-tools) available in `PATH`.
- A connected **device or emulator** with USB debugging authorized.

## Build

```bash
cargo build --release          # both binaries → target/release/{logstream,uitool}
cargo build -p logstream       # build a single tool
cargo test  --workspace        # unit tests (no device needed)
```

Install into `~/.cargo/bin`:

```bash
cargo install --path logstream
cargo install --path uitool
```

## Quick start

```bash
# Follow one app's logs in a live TUI (default package is overridden with -p)
logstream -p com.example.myapp
logstream -p com.example.myapp --json      # one JSON object per line
logstream -p com.example.myapp --plain -o run.log

# Inspect and drive the screen
uitool dump                                # label + bounds of every labeled node
uitool find "Settings" --clickable         # matching nodes, exit 1 if none
uitool tap "Settings"                      # tap by label (or: uitool tap 978 2316)
uitool shot --crop 0,100,1080,400 --scale 2
```

Full usage: [`logstream/README.md`](logstream/README.md) and [`uitool/README.md`](uitool/README.md).

## Repository layout

```
.
├── Cargo.toml          # Cargo workspace (members: adbkit, logstream, uitool)
├── Cargo.lock          # single lockfile for the whole workspace
├── index.json          # machine-readable catalog of the crates (descriptions + metadata)
├── README.md           # this file
├── LICENSE             # MIT (also duplicated inside each crate folder)
├── adbkit/             # shared adb helpers (library)
│   ├── Cargo.toml
│   ├── LICENSE
│   ├── README.md
│   └── src/lib.rs
├── logstream/          # tool 1 — log streamer
│   ├── Cargo.toml
│   ├── LICENSE
│   ├── README.md
│   └── src/
└── uitool/             # tool 2 — UI driver
    ├── Cargo.toml
    ├── LICENSE
    ├── README.md
    └── src/
```

## index.json

[`index.json`](index.json) is the machine-readable catalog: one entry per tool with
version, license, description, tags, CLI metadata, dependencies and links to its
README/LICENSE. Use it to script discovery, generate docs, or feed a tool registry:

```bash
jq -r '.tools[] | "\(.name)\t\(.version)\t\(.description)"' index.json
```

Keep it in sync when a tool's version, description or CLI surface changes.

## License

MIT — see [`LICENSE`](LICENSE). Each tool folder carries its own copy of the same
license (`logstream/LICENSE`, `uitool/LICENSE`).
