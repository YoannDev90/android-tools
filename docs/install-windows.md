# Install on Windows

## Whole toolkit (selector)

```powershell
irm https://raw.githubusercontent.com/YoannDev90/android-tools/main/scripts/install.ps1 | iex
```

With arguments, wrap the download in a scriptblock:

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/YoannDev90/android-tools/main/scripts/install.ps1))) update
```

Installs to `%LOCALAPPDATA%\android-tools\bin` (added to your user PATH).
Accepts `install [tool…]`, `update`, `list`, `uninstall`, `-All`, `-BinDir`.

## Single tool

```powershell
irm https://github.com/YoannDev90/android-tools/releases/latest/download/apkctl-installer.ps1 | iex
```

Other channels:

```powershell
# Windows package manager
winget install YoannDev90.apkctl

# crates.io (needs Rust)
cargo install apkctl
```

Replace `apkctl` with any tool from the README table. See [RELEASING.md](../RELEASING.md) for the full chain.
