# Install on Linux

## Whole toolkit (selector)

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://raw.githubusercontent.com/YoannDev90/android-tools/main/scripts/install.sh | bash
```

Updater / list / uninstall:

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://raw.githubusercontent.com/YoannDev90/android-tools/main/scripts/install.sh \
  | bash -s -- update
```

Both accept `install [tool…]`, `update`, `list`, `uninstall`, `--all` / `--bin-dir`.

## Single tool

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/YoannDev90/android-tools/releases/latest/download/apkctl-installer.sh | sh
```

Other channels:

```bash
# Debian / Ubuntu
sudo apt install ./apkctl_*_amd64.deb

# Fedora / RHEL
sudo dnf install ./apkctl-*.rpm

# crates.io (needs Rust)
cargo install apkctl

# Linuxbrew
brew install YoannDev90/homebrew-tap/apkctl
```

Replace `apkctl` with any tool from the README table. See [RELEASING.md](../RELEASING.md) for the full chain.
