#!/usr/bin/env python3
"""Regenerate the dynamic portions of README.md from index.json.

Only the content between <!-- BEGIN:name --> / <!-- END:name --> markers is
replaced; the rest of README.md is left untouched so prose can be edited by
hand. Sections regenerated: tools-table.
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

    with open(output_path, "w", encoding="utf-8") as f:
        f.write(text)
    print(f"Updated {output_path}")


if __name__ == "__main__":
    root_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    output_path = os.path.join(root_dir, "README.md")
    generate_readme(root_dir, output_path)
