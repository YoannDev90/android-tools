#!/usr/bin/env python3
"""Generate index.json from .METADATA.toml files in each tool directory."""

import json
import os
import tomllib
from datetime import datetime, timezone


def count_loc(tool_dir, files):
    """Count total lines across the given source files.

    Paths in .METADATA.toml are relative to the crate: resolved against
    src/ first (Rust convention), then the crate root.
    """
    total = 0
    for f in files:
        path = os.path.join(tool_dir, "src", f)
        if not os.path.isfile(path):
            path = os.path.join(tool_dir, f)
        if os.path.isfile(path):
            with open(path, "r", encoding="utf-8", errors="replace") as fh:
                total += sum(1 for _ in fh)
    return total


def collect_tools(root_dir):
    tools = []
    for entry in sorted(os.listdir(root_dir)):
        tool_dir = os.path.join(root_dir, entry)
        if not os.path.isdir(tool_dir):
            continue
        metadata_path = os.path.join(tool_dir, ".METADATA.toml")
        if not os.path.isfile(metadata_path):
            continue

        with open(metadata_path, "rb") as f:
            meta = tomllib.load(f)

        source = meta["metadata"]["source"]
        source["loc"] = count_loc(tool_dir, source.get("files", []))

        tool_entry = {
            "id": meta["package"]["name"],
            "name": meta["package"]["name"],
            "path": entry,
            "version": meta["package"]["version"],
            "edition": meta["package"]["edition"],
            "language": "rust",
            "license": meta["package"]["license"],
            "description": meta["package"]["description"],
            "long_description": meta["metadata"]["long_description"],
            "readme": meta["metadata"]["readme"],
            "license_file": meta["metadata"]["license_file"],
            "manifest": meta["metadata"]["manifest"],
            "entrypoint": meta["metadata"]["entrypoint"],
            "category": meta["metadata"]["category"],
            "tags": meta["metadata"]["tags"],
            "requires": meta["metadata"]["requires"],
            "features": meta["metadata"]["features"],
            "dependencies": meta["metadata"]["dependencies"],
            "tests": meta["metadata"]["tests"],
            "source": source,
        }
        tools.append(tool_entry)

    return tools


def generate_index(root_dir, output_path):
    tools = collect_tools(root_dir)

    index = {
        "name": "android-tools",
        "version": "0.1.0",
        "description": "adb-based tools for Android development, written in Rust.",
        "license": "MIT",
        "language": "rust",
        "edition": "2021",
        "catalog_updated": datetime.now(timezone.utc).strftime("%Y-%m-%d"),
        "repository": {
            "type": "git",
            "root": ".",
            "workspace": True,
            "manifest": "Cargo.toml",
            "readme": "README.md",
            "license_file": "LICENSE",
        },
        "requirements": {
            "rust": "stable (verified with rustc 1.99)",
            "adb": "Android SDK platform-tools in PATH",
            "device": "Connected Android device or emulator with USB debugging authorized",
        },
        "build": {
            "tool": "cargo",
            "release": "cargo build --release",
            "test": "cargo test --workspace",
            "binaries_dir": "target/release",
        },
        "tools": tools,
    }

    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(index, f, indent=2, ensure_ascii=False)
        f.write("\n")

    print(f"Generated {output_path} with {len(tools)} tools")


if __name__ == "__main__":
    root_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    output_path = os.path.join(root_dir, "index.json")
    generate_index(root_dir, output_path)
