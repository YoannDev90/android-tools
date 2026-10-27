#!/usr/bin/env python3
"""Fetch download counts per tool and write stats.json.

Merges GitHub Releases (summed over every asset whose name starts with the
tool, which covers tarballs, zips, .deb, .rpm and the installers) with
crates.io. The README's shields.io badges read stats.json live, so nothing
numeric lands in README.md itself.

Kept out of generate_readme.py on purpose: this one needs the network, while
CI regenerates the README offline and fails when it differs from the
committed copy.

Refreshed by .github/workflows/stats.yml. The file is only rewritten when a
count actually moved, so a routine run leaves git with nothing to commit.
"""

import json
import os
import re
import sys
import urllib.error
import urllib.request
from datetime import date

REPO = "YoannDev90/android-tools"
RAW = f"https://raw.githubusercontent.com/{REPO}/main/stats.json"
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
USER_AGENT = "android-tools-stats/0.1"


def fetch(url, headers):
    request = urllib.request.Request(url, headers=headers)
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.load(response)


def github_counts(tools):
    """Release assets are named `<tool>-...` or `<tool>_...`. Everything else
    — checksums, dist-manifest, source tarball, Homebrew formulas — carries no
    tool prefix and is skipped."""
    counts = {tool: 0 for tool in tools}
    headers = {"User-Agent": USER_AGENT, "Accept": "application/vnd.github+json"}
    token = os.environ.get("GITHUB_TOKEN")
    if token:
        headers["Authorization"] = f"Bearer {token}"

    page = 1
    while True:
        releases = fetch(
            f"https://api.github.com/repos/{REPO}/releases?per_page=100&page={page}",
            headers,
        )
        if not releases:
            break
        for release in releases:
            for asset in release["assets"]:
                tool = re.split(r"[-_]", asset["name"], maxsplit=1)[0]
                if tool in counts:
                    counts[tool] += asset["download_count"]
        if len(releases) < 100:
            break
        page += 1
    return counts


def crates_counts(tools):
    counts = {}
    headers = {"User-Agent": USER_AGENT}
    for tool in tools:
        try:
            counts[tool] = fetch(f"https://crates.io/api/v1/crates/{tool}", headers)[
                "crate"
            ]["downloads"]
        except urllib.error.HTTPError as exc:
            # A crate added to the catalog but not published yet.
            if exc.code == 404:
                counts[tool] = 0
            else:
                raise
    return counts


def main():
    with open(os.path.join(ROOT, "index.json"), encoding="utf-8") as f:
        tools = [t["name"] for t in json.load(f)["tools"]]

    github = github_counts(tools)
    crates = crates_counts(tools)

    stats = {"total": 0, "tools": {}}
    for tool in sorted(tools):
        entry = {
            "github": github[tool],
            "crates_io": crates[tool],
            "total": github[tool] + crates[tool],
        }
        stats["tools"][tool] = entry
        stats["total"] += entry["total"]

    out_path = os.path.join(ROOT, "stats.json")
    previous = {}
    if os.path.isfile(out_path):
        try:
            with open(out_path, encoding="utf-8") as f:
                previous = json.load(f)
        except json.JSONDecodeError:
            previous = {}

    if previous.get("tools") == stats["tools"] and previous.get("total") == stats["total"]:
        print("stats.json already current — leaving it untouched")
        return 0

    stats["updated"] = date.today().isoformat()
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(stats, f, indent=2)
        f.write("\n")

    print(f"Generated {out_path} — {stats['total']} downloads across {len(tools)} tools")
    for tool, entry in stats["tools"].items():
        print(f"  {tool:<12} github={entry['github']:>4}  crates.io={entry['crates_io']:>4}  total={entry['total']:>4}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
