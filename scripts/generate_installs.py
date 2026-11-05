#!/usr/bin/env python3
"""Inline install/utils/* into install/install.{sh,ps1} for distribution.

The decomposed sources live in install/src/ (with `.`/dot-source lines
pointing at install/src/utils/). This script produces the self-contained
install/install.sh and install/install.ps1 that are served via curl|bash
and irm|iex. Run after editing any source or util.
"""

import os
import re

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC_DIR = os.path.join(ROOT, "install", "src")
UTILS_DIR = os.path.join(SRC_DIR, "utils")
OUT_DIR = os.path.join(ROOT, "install")


def inline_sh(text):
    pat = re.compile(r'^\. "\$\(dirname "\$0"\)/\.\./utils/(\w+)\.sh"\s*$', re.M)

    def repl(m):
        name = m.group(1)
        path = os.path.join(UTILS_DIR, name + ".sh")
        content = open(path, encoding="utf-8").read()
        lines = content.splitlines()
        # strip shebang and leading blank lines
        while lines and (lines[0].startswith("#!") or lines[0].strip() == ""):
            lines.pop(0)
        content = "\n".join(lines).rstrip()
        return inline_sh(content)

    # Repeat until no source lines remain (handles nested inlining).
    while pat.search(text):
        text = pat.sub(repl, text, count=1)
    return text


def inline_ps1(text):
    pat = re.compile(r'^\. "\$PSScriptRoot\\\.\.\\utils\\(\w+)\.ps1"\s*$', re.M)

    def repl(m):
        name = m.group(1)
        path = os.path.join(UTILS_DIR, name + ".ps1")
        content = open(path, encoding="utf-8").read()
        lines = content.splitlines()
        while lines and (lines[0].startswith("#!") or lines[0].strip() == ""):
            lines.pop(0)
        content = "\n".join(lines).rstrip()
        return inline_ps1(content)

    while pat.search(text):
        text = pat.sub(repl, text, count=1)
    return text


def main():
    sh_src = open(os.path.join(SRC_DIR, "install.sh"), encoding="utf-8").read()
    ps1_src = open(os.path.join(SRC_DIR, "install.ps1"), encoding="utf-8").read()

    sh_out = inline_sh(sh_src)
    ps1_out = inline_ps1(ps1_src)

    with open(os.path.join(OUT_DIR, "install.sh"), "w", encoding="utf-8", newline="\n") as f:
        f.write(sh_out)
    with open(os.path.join(OUT_DIR, "install.ps1"), "w", encoding="utf-8", newline="\n") as f:
        f.write(ps1_out)

    print("Generated install/install.sh and install/install.ps1")


if __name__ == "__main__":
    main()
