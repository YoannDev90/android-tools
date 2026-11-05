#!/usr/bin/env python3
"""Regenerate the dynamic portions of README.md from index.json.

Only the content between <!-- BEGIN:name --> / <!-- END:name --> markers is
replaced; the rest of README.md is left untouched so prose can be edited by
hand. Sections regenerated: tools-table, layout.
"""

import json
import os
import re
from urllib.parse import quote

STATS_JSON = "https://raw.githubusercontent.com/YoannDev90/android-tools/main/stats.json"


def tool_badge(tool):
    return (
        "https://img.shields.io/badge/dynamic/json"
        f"?url={quote(STATS_JSON, safe='')}"
        f"&query=$.tools.{tool}.total"
        f"&label={tool}"
        "&style=for-the-badge"
    )


def render_tools_table(tools):
    lines = [
        "| Crate | Description | Downloads |",
        "|-------|-------------|-----------|",
    ]
    for t in tools:
        lines.append(
            f"| [`{t['name']}`]({t['path']}/) | {t['description']} | "
            f"![{t['name']}]({tool_badge(t['name'])}) |"
        )
    return "\n".join(lines)


def render_layout(root_dir):
    EXCLUDED = {"Ideas.md", ".gitignore", ".git", "target", "__pycache__"}
    ROOT_FILES = ["Cargo.toml", "Cargo.lock", "index.json", "stats.json", "README.md", "LICENSE", "rustfmt.toml"]

    def children(path):
        return sorted(e for e in os.listdir(path) if e not in EXCLUDED and e != "target")

    entries = []
    for name in ROOT_FILES:
        if os.path.isfile(os.path.join(root_dir, name)):
            entries.append((name, False))
    for name in children(root_dir):
        if os.path.isdir(os.path.join(root_dir, name)):
            entries.append((name, True))

    lines = ["."]
    for i, (name, is_dir) in enumerate(entries):
        last = i == len(entries) - 1
        lines.append(("└── " if last else "├── ") + name + ("/" if is_dir else ""))
        if not is_dir:
            continue
        prefix = "    " if last else "│   "
        subs = children(os.path.join(root_dir, name))
        for j, s in enumerate(subs):
            sub_last = j == len(subs) - 1
            lines.append(prefix + ("└── " if sub_last else "├── ") + s)
            if name != "src" or not os.path.isdir(os.path.join(root_dir, name, s)):
                continue
            inner = children(os.path.join(root_dir, name, s))
            inner_prefix = prefix + ("    " if sub_last else "│   ")
            for k, t in enumerate(inner):
                inner_last = k == len(inner) - 1
                lines.append(inner_prefix + ("└── " if inner_last else "├── ") + t)
    return "```\n" + "\n".join(lines) + "\n```"


def replace_section(text, name, content):
    pattern = re.compile(rf"<!-- BEGIN:{name} -->.*?<!-- END:{name} -->", re.DOTALL)
    replacement = f"<!-- BEGIN:{name} -->\n{content}\n<!-- END:{name} -->"
    new, n = pattern.subn(replacement, text)
    if n != 1:
        raise SystemExit(f"expected exactly one {name} marker block, found {n}")
    return new


def generate_readme(root_dir, output_path):
    with open(os.path.join(root_dir, "index.json"), encoding="utf-8") as f:
        index = json.load(f)
    tools = index.get("tools", [])

    with open(output_path, encoding="utf-8") as f:
        text = f.read()

    text = replace_section(text, "tools-table", render_tools_table(tools))
    text = replace_section(text, "layout", render_layout(root_dir))

    with open(output_path, "w", encoding="utf-8") as f:
        f.write(text)
    print(f"Updated {output_path}")


if __name__ == "__main__":
    root_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    output_path = os.path.join(root_dir, "README.md")
    generate_readme(root_dir, output_path)
