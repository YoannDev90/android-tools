#!/usr/bin/env bash
# Install an android-tools agent skill into the canonical skills directory
# (~/.agents/skills/<tool>/SKILL.md) and symlink it into any detected agent
# dirs (~/.claude, ~/.cursor, ~/.codex, ...).
#
# Usage: install-skills.sh <tool> <path-to-SKILL.md>
set -euo pipefail

tool="${1:?tool name required}"
skill_file="${2:?skill file required}"

agents_home="${AGENTS_HOME:-$HOME/.agents}"
canonical="$agents_home/skills/$tool"
mkdir -p "$canonical"
cp "$skill_file" "$canonical/SKILL.md"

for agent in claude cursor codex gemini; do
    agent_dir="$HOME/.$agent"
    [ -d "$agent_dir" ] || continue
    mkdir -p "$agent_dir/skills"
    link="$agent_dir/skills/$tool"
    [ -e "$link" ] && rm -rf "$link"
    ln -s "$canonical" "$link"
done

echo "Installed skill for $tool -> $canonical"
