#!/usr/bin/env python3
"""Generate an nfpm.yaml per tool, for the .deb/.rpm packages.

Reads index.json (the generated catalog) and writes one nfpm config per
dist-able tool into the directory given as the first argument. adbkit is
skipped: it is a library, not a binary.

Usage: python3 scripts/make_nfpm.py <out-dir> <version> <source-file> <arch>
"""

import json
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def main():
    out_dir, version, source, arch = sys.argv[1:5]

    with open(os.path.join(ROOT, "index.json"), encoding="utf-8") as f:
        tools = json.load(f)["tools"]

    os.makedirs(out_dir, exist_ok=True)
    written = []
    for tool in tools:
        if tool["category"] == "library":
            continue
        name = tool["name"]
        config = f"""name: {name}
arch: {arch}
platform: linux
version: {version}
section: devel
priority: optional
maintainer: {tool.get('maintainer', 'YoannDev90')}
description: |
  {tool['description']}
license: {tool['license']}
homepage: https://github.com/YoannDev90/android-tools
contents:
  - src: {source}/{name}
    dst: /usr/bin/{name}
"""
        path = os.path.join(out_dir, f"{name}.nfpm.yaml")
        with open(path, "w", encoding="utf-8") as f:
            f.write(config)
        written.append(name)

    print(f"wrote {len(written)} nfpm configs: {', '.join(written)}")


if __name__ == "__main__":
    main()
