# resclean

Dead `res/` resource detector — find what an Android project defines but
never uses, and gate CI on it.

Walks a project (or module) root, collects every resource it defines —
files under `res/` and entries in `values/*.xml` — and every reference to
them from layouts, menus, manifests, other `res/` XML, and Kotlin/Java
code (`R.*` / `R2.*`). Anything defined but never referenced is reported.

## Requirements

- None. Pure source scan: no device, no `aapt`, no Gradle.

## Install

```bash
cargo install --path .        # from this folder
# or, from the repository root:
cargo install --path resclean
```

## Usage

```bash
resclean .                              # scan the current project
resclean app/ --json                    # machine-readable report
resclean app/ --keep 'string/get'       # ignore getIdentifier() lookups
resclean app/ --keep 'drawable/ic_' --keep 'string/app_'
resclean app/ --warn-only               # report but never fail
```

`--keep` is a regex matched against `kind/name` (repeatable). Use it for
resources reached dynamically (`getIdentifier("name", "drawable", …)`),
which no text scan can see.

### Exit codes

| Code | Meaning |
|------|---------|
| `0` | No dead resource — or `--warn-only`, whatever the report |
| `1` | At least one dead resource — the CI gate |
| `2` | Error (unreadable path, bad `--keep` regex…) |

## Output

```text
3 dead resources (10 defined, 9 referenced)
drawable/ (1):
  drawable/orphan
layout/ (1):
  layout/unused_layout
string/ (1):
  string/unused
```

`--json` reports `dead` (kind → names), `defined`, `referenced`,
`total`, `failed`.

## Method notes

- A resource is defined by a file (`res/drawable-xhdpi/row.png` →
  `drawable/row`; qualifiers and `.9.png` are normalized away) or by a
  values entry (`<string name="hello">`, `<style>`, `<item type="…">`).
- References come from `@type/name` in XML and `R.type.name` /
  `R2.type.name` in code. `@+id/…` **defines** an id (so an unused id is
  reported); a plain `@id/…` or `R.id.…` references one.
- Style names are matched across the code/XML divide: `R.style.Theme_X`
  = `@style/Theme.X`.
- Every limit is in the *never-flag-a-live-resource* direction: `tools:`
  preview attributes count as used; `attr` is never reported (it is
  referenced through the `app:` namespace); a resource referenced only
  by a dead one still counts as used (no cascade analysis);
  `getIdentifier()` lookups need `--keep`.
- `build/`, `.gradle/`, `.git/` and similar dirs are skipped.

## Development

```bash
cargo test          # 7 unit tests — name/kind normalization, values & XML
                    # parsing, tools: namespace, R/R2 code refs, a tiny
                    # fixture project; no device or Android SDK needed
cargo clippy
cargo build --release
```

Source layout: `main.rs` (CLI and reports), `scan.rs` (the analysis and
its tests). The fixture project lives in `tests/fixtures/tiny`.

## License

MIT — see [LICENSE](LICENSE).
