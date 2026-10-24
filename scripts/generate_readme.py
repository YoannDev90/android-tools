#!/usr/bin/env python3
"""Generate README.md from index.json."""

import json
import os

ROOT_FILES = ["Cargo.toml", "Cargo.lock", "index.json", "README.md", "LICENSE", "rustfmt.toml"]
ROOT_DIRS = ["adbkit", "logstream", "startperf", "uitool", "scripts"]
# Personal scratch notes / local tooling — not part of the generated project docs.
EXCLUDED = {"Ideas.md", ".gitignore", ".git", "target"}


def render_layout(root_dir):
    """ASCII tree of the repository, derived from index.json tools + root files."""
    tools = []
    if os.path.isfile(os.path.join(root_dir, "index.json")):
        with open(os.path.join(root_dir, "index.json"), encoding="utf-8") as f:
            tools = [t for t in json.load(f)["tools"] if t.get("category") != "library"]

    entries = []
    for name in ROOT_FILES:
        if os.path.isfile(os.path.join(root_dir, name)):
            entries.append((name, os.path.isdir(os.path.join(root_dir, name))))
    for name in sorted(os.listdir(root_dir)):
        path = os.path.join(root_dir, name)
        if os.path.isdir(path) and name not in EXCLUDED and name not in ROOT_DIRS:
            entries.append((name, True))
    for name in ROOT_DIRS:
        if os.path.isdir(os.path.join(root_dir, name)):
            entries.append((name, True))

    lines = ["."]
    for i, (name, is_dir) in enumerate(entries):
        last = i == len(entries) - 1
        lines.append(("└── " if last else "├── ") + name + ("/" if is_dir else ""))
        if is_dir:
            sub = sorted(
                e for e in os.listdir(os.path.join(root_dir, name))
                if e not in EXCLUDED and e != "target"
            )
            for j, s in enumerate(sub):
                sub_last = j == len(sub) - 1
                prefix = "    " if last else "│   "
                lines.append(prefix + ("└── " if sub_last else "├── ") + s)
    return "\n".join(lines)


def generate_readme(root_dir, output_path):
    with open(os.path.join(root_dir, "index.json"), encoding="utf-8") as f:
        index = json.load(f)

    name = index.get("name", "android-tools")
    description = index.get("description", "")
    tools = index.get("tools", [])
    reqs = index.get("requirements", {})
    build = index.get("build", {})

    lines = [f"# {name}", "", description, ""]

    # Tools table
    if tools:
        lines += [
            "| Crate | Description |",
            "|-------|-------------|",
        ]
        for t in tools:
            lines.append(f"| [`{t['name']}`]({t['path']}/) | {t['description']} |")
        lines.append("")

    # Requirements
    lines.append("## Requirements")
    if "rust" in reqs:
        lines.append(f"- **Rust**: {reqs['rust']}")
    if "adb" in reqs:
        lines.append(f"- **adb**: {reqs['adb']}")
    if "device" in reqs:
        lines.append(f"- **Device**: {reqs['device']}")
    lines.append("")

    # Install (distribution channels set up in RELEASING.md)
    lines += [
        "## Install",
        "",
        "Binaries for Linux (musl, any distro), Windows and macOS are attached to",
        "each [release](https://github.com/YoannDev90/android-tools/releases). Pick",
        "a tool, then:",
        "",
        "```bash",
        "# Linux / macOS — detects OS and arch, installs to ~/.local/bin:",
        "curl --proto '=https' --tlsv1.2 -LsSf \\",
        "  https://github.com/YoannDev90/android-tools/releases/latest/download/apkctl-installer.sh | sh",
        "",
        "# Windows (PowerShell):",
        "irm https://github.com/YoannDev90/android-tools/releases/latest/download/apkctl-installer.ps1 | iex",
        "",
        "# macOS / Linuxbrew:",
        "brew install YoannDev90/homebrew-tap/apkctl",
        "",
        "# Debian / Ubuntu (.deb) and Fedora / RHEL (.rpm) attached to the release:",
        "sudo apt install ./apkctl_*_amd64.deb",
        "",
        "# From crates.io (needs Rust):",
        "cargo install apkctl",
        "",
        "# Windows package manager:",
        "winget install YoannDev90.apkctl",
        "```",
        "",
        "Replace `apkctl` with any tool from the table above. See",
        "[`RELEASING.md`](RELEASING.md) for the full release and packaging chain.",
        "",
    ]

    # Build
    lines.append("## Build")
    if "release" in build:
        cmds = [build["release"], build.get("test", "")]
        if build.get("fmt"):
            cmds.append(build["fmt"])
        if build.get("clippy"):
            cmds.append(build["clippy"])
        lines.append("```bash\n" + "\n".join(c for c in cmds if c) + "\n```")
    lines.append("")

    # Scripts
    lines += [
        "## Code generation",
        "",
        "`index.json` and this README are generated — edit the sources, then:",
        "",
        "```bash",
        "python3 scripts/generate_index.py   # .METADATA.toml -> index.json",
        "python3 scripts/generate_readme.py  # index.json -> README.md",
        "```",
        "",
        "`.METADATA.toml` lives at the root of each tool directory and holds the",
        "tool description, tags, requirements and dependency metadata.",
        "",
        "CI (`.github/workflows/ci.yml`) runs fmt/clippy/test and fails when these",
        "generated files are out of date with their sources.",
        "",
    ]

    # Layout
    lines += ["## Repository layout", "", "```", render_layout(root_dir), "```", ""]

    # License
    lines += [
        "## License",
        "",
        "MIT — see [LICENSE](LICENSE). Each crate folder carries its own copy.",
        "",
    ]

    with open(output_path, "w", encoding="utf-8") as f:
        f.write("\n".join(lines))

    print(f"Generated {output_path}")


if __name__ == "__main__":
    root_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    output_path = os.path.join(root_dir, "README.md")
    generate_readme(root_dir, output_path)
