# Releasing

How a version goes from a git tag to being installable on Linux, Windows and
macOS. The whole chain is triggered by **one tag**.

## What a release produces

For every tool (all of them, in lockstep — one version across the workspace):

| Channel | Content |
|---|---|
| GitHub Release | archives per target, `.sha256` checksums, `*-installer.sh`, `*-installer.ps1` |
| Linux | `x86_64` + `aarch64` **musl static** archives (run on any distro, no glibc issues) |
| Windows | `x86_64` + `aarch64` MSVC archives |
| macOS | `x86_64` + `aarch64` darwin archives |
| Homebrew | `YoannDev90/homebrew-tap`, fed automatically by dist |
| .deb / .rpm | `amd64` + `arm64`, attached to the same release (`packages.yml`) |
| crates.io | every crate, `adbkit` first (`publish-crates.yml`) |
| winget | one manifest per tool (`winget.yml`) |

## Cut a release

1. Make sure `main` is green (CI: fmt, clippy, tests, generated files).
2. Bump the version **in every crate** — they move in lockstep. The tag must
   match the crate versions or `publish-crates.yml` fails loudly:

   ```bash
   for c in adbkit apkctl assetgen dp2px layoutlint logstream perfpoll \
            resclean scenediff startperf svg2vd uitool; do
     sed -i "s/^version = \".*\"/version = \"X.Y.Z\"/" $c/Cargo.toml
   done
   ```

   `.METADATA.toml` files carry their own `version` field for `index.json` —
   bump those too, then regenerate:

   ```bash
   python3 scripts/generate_index.py
   python3 scripts/generate_readme.py
   ```

3. Commit, then tag and push:

   ```bash
   git commit -am "release: vX.Y.Z"
   git tag vX.Y.Z
   git push origin main vX.Y.Z
   ```

4. The chain runs by itself:
   - `release.yml` (dist) builds all targets, creates the GitHub Release with
     archives, checksums and installers, and pushes to the Homebrew tap.
   - Once that run finishes, `packages.yml` builds `.deb`/`.rpm` and attaches
     them, `publish-crates.yml` publishes `adbkit` then every tool to
     crates.io, and `winget.yml` opens the winget-pkgs PRs.

   The three start on the Release run's **completion** (`workflow_run`), not on
   `release: published`: dist creates the release with `GITHUB_TOKEN`, and
   GitHub never fires a workflow for an event caused by `GITHUB_TOKEN` (only
   `workflow_dispatch`/`repository_dispatch` are exempt). They therefore also
   have a manual *Run workflow* entry point that takes the tag, e.g. `v0.1.0`.

Preview a release without publishing: `dist plan`.

## Required secrets

| Secret | Used by | What it is |
|---|---|---|
| `HOMEBREW_TAP_TOKEN` | `release.yml` | PAT with write access to the `YoannDev90/homebrew-tap` repo (create it empty; dist pushes the formulas) |
| `CARGO_REGISTRY_TOKEN` | `publish-crates.yml` | crates.io API token (`cargo login` → `~/.cargo/credentials.toml`) |
| `WINGET_TOKEN` | `winget.yml` | PAT from an account that forked `microsoft/winget-pkgs`, with `workflow` scope |

`GITHUB_TOKEN` is enough for everything else (dist and the package upload use
the built-in token with `contents: write`).

## Install story (what users run)

```bash
# any OS, from the GitHub Release (arch-aware):
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/YoannDev90/android-tools/releases/latest/download/apkctl-installer.sh | sh
# Windows:
irm https://github.com/YoannDev90/android-tools/releases/latest/download/apkctl-installer.ps1 | iex

# macOS / Linuxbrew:
brew install YoannDev90/homebrew-tap/apkctl

# Debian / Ubuntu:
sudo apt install ./apkctl_0.1.0_amd64.deb

# Fedora / RHEL:
sudo rpm -i apkctl-0.1.0.x86_64.rpm

# anywhere with Rust:
cargo install apkctl

# Windows:
winget install YoannDev90.apkctl
```

## Notes

- **crates.io names are permanent.** `appctl` and `dpx` were taken, hence
  `apkctl` and `dp2px`; the binaries keep those names.
- The Linux binaries are linked against **musl**, statically: one binary runs
  on Debian, Ubuntu, Alpine, Fedora, Arch… without rebuilds.
- `pr-run-mode = "plan"` in `dist-workspace.toml` makes PRs run `dist plan`, so
  a broken dist config fails before the tag, not after it.
