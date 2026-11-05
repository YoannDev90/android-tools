#!/usr/bin/env bash
info() { printf '%s\n' "$*"; }
ok() { printf '%s ok%s %s\n' "$C_GREEN" "$C_OFF" "$*"; }
warn() { printf '%swarning:%s %s\n' "$C_YELLOW" "$C_OFF" "$*" >&2; }
die() {
	printf '%serror:%s %s\n' "$C_RED" "$C_OFF" "$*" >&2
	exit 1
}

usage() {
	cat <<EOF
android-tools installer — pick the tools you want, keep them updated.

Usage: install.sh [command] [options] [tool...]

Commands:
  install [tool... | all]    install tools (interactive selector by default)
  update  [tool... | all]    update the tools already installed
  list                       show installed / available tools
  uninstall [tool... | all]  remove tools
  help                       print this help

Options:
  --all            every tool, no selector
  --bin-dir DIR    install directory (default: $HOME/.local/bin)
  -h, --help       print this help

Examples:
  install.sh                          # selector
  install.sh install all              # everything, no prompt
  install.sh install apkctl svg2vd    # a subset
  install.sh update                   # updater for what is installed
  install.sh list
  install.sh uninstall apkctl

Environment: ANDROID_TOOLS_BIN, ANDROID_TOOLS_STATE, NO_COLOR.
EOF
}

# ------------------------------------------------------------------ network
