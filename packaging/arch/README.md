# Arch packages

One source PKGBUILD per tool, one directory each — feed them to your
`[AUR-bin]` repo (or AUR) as-is.

```
packaging/arch/<tool>/PKGBUILD
```

They are **generated**, do not edit them by hand:

```bash
python3 scripts/make_pkgbuild.py
```

The script reads `index.json`, skips `adbkit` (library, not a binary) and
downloads the release-tag tarball to compute `sha256sums` — so run it **after**
the tag is pushed (step 3 of [`RELEASING.md`](../../RELEASING.md)); it exits
with an error if `v<pkgver>` does not exist yet.

Each PKGBUILD builds only its own crate out of the workspace tarball
(`cargo build --frozen --release -p <tool>`), runs `cargo test`, then installs:

| Path | Content |
|---|---|
| `/usr/bin/<tool>` | the binary |
| `/usr/share/doc/<tool>/README.md` | the tool's README |
| `/usr/share/licenses/<tool>/LICENSE` | MIT |

Tools that link `adbkit` declare `depends=('android-tools')` (Arch's package
providing `adb`); the rest are standalone.

Build one locally to verify:

```bash
cd packaging/arch/dp2px && makepkg -sf
```
