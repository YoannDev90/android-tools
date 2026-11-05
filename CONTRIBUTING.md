# Contributing

## Build

```bash
cargo build --release
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

## Code generation

`index.json`, `README.md` and `stats.json` are generated — edit the
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
