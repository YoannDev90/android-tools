# AGENTS.md

Guidance for AI coding agents working in the `android-tools` repository.

## Skills

Per-tool agent skills live in `skills/` (one file per crate). Read the relevant one before invoking a tool.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for build commands and code-generation workflow. Follow it.

## Project overview

A Rust workspace of adb-based CLI tools for Android development. Each tool is its own crate under `src/`. `adbkit` is a shared library crate of adb helpers; the rest are binaries.

- Language: Rust (stable, verified rustc 1.99)
- Workspace: `Cargo.toml` at repo root, resolver 2, 13 members
- External requirement: `adb` (Android SDK platform-tools) on PATH, plus a connected device/emulator for device-touching tools
- License: MIT (root + per-crate)

## Layout

```
src/<crate>/{.METADATA.toml, Cargo.toml, LICENSE, README.md, src/}
scripts/          # installers + code generators
packaging/arch/    # Arch PKGBUILDs
.github/workflows # ci.yml, stats.yml
```

Generated files (do not hand-edit): `index.json`, `README.md`, `stats.json`. They are produced by `scripts/generate_index.py`, `generate_readme.py`, `generate_stats.py` from `.METADATA.toml`. CI fails if they are stale.

## Build, test, lint

```bash
cargo build --release
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

Run from repo root. `cargo test --workspace` covers all crates including `resclean` integration tests (fixtures in `src/resclean/tests/fixtures`).

## Conventions

- Formatting: `rustfmt.toml` at root; run `cargo fmt --all` before committing.
- Clippy must pass with `-D warnings` (no warnings allowed).
- Each crate folder carries its own `LICENSE` and `README.md`.
- Tool metadata (description, tags, requirements, dependencies) lives in `src/<crate>/.METADATA.toml`, not in README. Edit there, then regenerate.
- Keep generated files in sync: after touching `.METADATA.toml` or sources affecting index, run the generators and commit their output.

## Adding / modifying a tool

1. Add crate under `src/<name>/` with `Cargo.toml`, `.METADATA.toml`, `LICENSE`, `README.md`, `src/`.
2. Register in root `Cargo.toml` workspace `members`.
3. Update packaging/installer scripts as needed (see `scripts/`, `packaging/`).
4. Regenerate `index.json` and `README.md`.

## Notes for agents

- Prefer editing existing files over creating new ones; don't create docs unless asked.
- The `target/` and `packaging/arch/*/src/`, `pkg/`, `.pkg.tar.zst`, `.tar.gz` artifacts are build outputs — don't edit.
- `adbkit` is a library; changes there affect all consumers — run full workspace tests.
- Device-dependent tests may require a connected emulator; most unit tests do not.
- Don't commit generated stats manually; `generate_stats.py` hits the network and runs on a schedule.
