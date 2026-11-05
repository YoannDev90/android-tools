#!/usr/bin/env bash
#
# android-tools — global installer with an interactive tool selector and a
# built-in updater.
#
#   curl --proto '=https' --tlsv1.2 -LsSf \
#     https://raw.githubusercontent.com/YoannDev90/android-tools/main/install/install.sh | bash
#
# Commands (the first argument; "install" is the default):
#   install [tool... | all]   select the tools to install (interactive)
#   update  [tool... | all]   refresh the tools already installed
#   list                      show installed and available tools
#   uninstall [tool... | all] remove tools
#   help
#
# Options:
#   --all             operate on every tool (skips the selector)
#   --bin-dir DIR     install directory (default: ~/.local/bin)
#   -h, --help        print this help
#
# Environment:
#   ANDROID_TOOLS_BIN     same as --bin-dir
#   ANDROID_TOOLS_STATE   state directory
#                         (default: ${XDG_STATE_HOME:-~/.local/state}/android-tools)
#   NO_COLOR=1            disable colours
#
# Needs: bash, tar (with xz), curl or wget. Checksums use sha256sum,
# shasum or openssl when one is available.

set -euo pipefail

REPO="YoannDev90/android-tools"
INDEX_URL="https://raw.githubusercontent.com/$REPO/main/index.json"
RELEASE_URL="https://github.com/$REPO/releases/latest/download"
# Kept in sync with index.json by hand: used when the catalog cannot be fetched
# (offline run, no python3/jq to parse it). Library crates stay out — they ship
# no binary.
FALLBACK_TOOLS="apkctl assetgen dp2px iconkit layoutlint logstream perfpoll resclean scenediff startperf svg2vd uitool"

BIN_DIR="${ANDROID_TOOLS_BIN:-$HOME/.local/bin}"
STATE_DIR="${ANDROID_TOOLS_STATE:-${XDG_STATE_HOME:-$HOME/.local/state}/android-tools}"
STATE_FILE="$STATE_DIR/installed"

ALL=0
TARGET=""
TMP_ROOT=""

TOOLS=()
DESCS=()
LATEST_VERSION=""

S_TOOL=()
S_VER=()
S_SHA=()

SEL=()
TARGETS=()
INSTALLED=()

if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
	C_BOLD=$'\033[1m'
	C_DIM=$'\033[2m'
	C_GREEN=$'\033[32m'
	C_YELLOW=$'\033[33m'
	C_RED=$'\033[31m'
	C_OFF=$'\033[0m'
else
	C_BOLD=""
	C_DIM=""
	C_GREEN=""
	C_YELLOW=""
	C_RED=""
	C_OFF=""
fi

# shellcheck source=utils/logging.sh
. "$(dirname "$0")/utils/logging.sh"
# shellcheck source=utils/common.sh
. "$(dirname "$0")/utils/common.sh"

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

# shellcheck source=utils/http.sh
. "$(dirname "$0")/utils/http.sh"

# shellcheck source=utils/common.sh
# (common already sourced above)
# shellcheck source=utils/hash.sh
. "$(dirname "$0")/utils/hash.sh"

# --------------------------------------------------------------- platform

# shellcheck source=utils/platform.sh
. "$(dirname "$0")/utils/platform.sh"

# ---------------------------------------------------------------- catalog

# shellcheck source=utils/catalog.sh
. "$(dirname "$0")/utils/catalog.sh"

# ------------------------------------------------------------- tool checks

# shellcheck source=utils/tools.sh
. "$(dirname "$0")/utils/tools.sh"

# shellcheck source=utils/install.sh
. "$(dirname "$0")/utils/install.sh"

path_note() {
	case ":$PATH:" in
	*":$BIN_DIR:"*) return 0 ;;
	esac
	warn "$BIN_DIR is not in your PATH — add it:"
	printf '    export PATH="%s:$PATH"   # ~/.bashrc or ~/.zshrc\n' "$BIN_DIR"
}

# ---------------------------------------------------------------- selector

prompt_read() { # variable name — reads the console, never stdin: with
	# `curl … | bash` stdin carries the script itself.
	if { : </dev/tty; } 2>/dev/null; then
		IFS= read -r "$1" </dev/tty || return 1
		return 0
	fi
	if [ -t 0 ]; then
		IFS= read -r "$1" || return 1
		return 0
	fi
	return 1
}

sel_has() {
	local i=0
	while [ "$i" -lt "${#SEL[@]}" ]; do
		if [ "${SEL[$i]}" = "$1" ]; then
			return 0
		fi
		i=$((i + 1))
	done
	return 1
}

sel_toggle() {
	local i=0 found=-1
	while [ "$i" -lt "${#SEL[@]}" ]; do
		if [ "${SEL[$i]}" = "$1" ]; then
			found=$i
			break
		fi
		i=$((i + 1))
	done
	if [ "$found" -ge 0 ]; then
		local new=()
		i=0
		while [ "$i" -lt "${#SEL[@]}" ]; do
			if [ "$i" != "$found" ]; then
				new[${#new[@]}]="${SEL[$i]}"
			fi
			i=$((i + 1))
		done
		SEL=("${new[@]+"${new[@]}"}")
	else
		SEL[${#SEL[@]}]="$1"
	fi
}

sel_all() {
	SEL=()
	local i=0
	while [ "$i" -lt "${#TOOLS[@]}" ]; do
		SEL[${#SEL[@]}]="$i"
		i=$((i + 1))
	done
}

# Fills TARGETS. Returns 1 when there is no terminal to prompt on.
selector() {
	if ! { : </dev/tty; } 2>/dev/null && [ ! -t 0 ]; then
		return 1
	fi

	SEL=()
	local i=0
	while [ "$i" -lt "${#TOOLS[@]}" ]; do
		if ! is_installed "${TOOLS[$i]}"; then
			SEL[${#SEL[@]}]="$i"
		fi
		i=$((i + 1))
	done

	local input tok mark extra toks
	while true; do
		printf '\n%sandroid-tools%s — install to %s%s%s\n' \
			"$C_BOLD" "$C_OFF" "$C_DIM" "$BIN_DIR" "$C_OFF"
		i=0
		while [ "$i" -lt "${#TOOLS[@]}" ]; do
			if sel_has "$i"; then
				mark="x"
			else
				mark=" "
			fi
			extra=""
			if is_installed "${TOOLS[$i]}"; then
				extra=" ${C_DIM}(installed)${C_OFF}"
			fi
			printf ' [%s] %2d  %-11s %s%s\n' \
				"$mark" "$((i + 1))" "${TOOLS[$i]}" "${DESCS[$i]}" "$extra"
			i=$((i + 1))
		done
		printf '\n numbers toggle (e.g. %s1 4 7%s), %sa%s=all, %sn%s=none, %sEnter%s=install, %sq%s=quit\n> ' \
			"$C_BOLD" "$C_OFF" "$C_BOLD" "$C_OFF" "$C_BOLD" "$C_OFF" \
			"$C_BOLD" "$C_OFF" "$C_BOLD" "$C_OFF"
		if ! prompt_read input; then
			return 1
		fi

		input="$(printf '%s' "$input" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')"
		if [ -z "$input" ]; then
			break
		fi

		toks=()
		read -ra toks <<<"$input"
		for tok in ${toks[@]+"${toks[@]}"}; do
			case "$tok" in
			q | quit | exit)
				info ""
				info "Aborted, nothing installed."
				exit 0
				;;
			a | all)
				sel_all
				;;
			n | none | clear)
				SEL=()
				;;
			\?)
				printf ' numbers toggle a=all n=none Enter=install q=quit\n'
				;;
			*[!0-9]*)
				if known_tool "$tok"; then
					i=0
					while [ "$i" -lt "${#TOOLS[@]}" ]; do
						if [ "${TOOLS[$i]}" = "$tok" ]; then
							sel_toggle "$i"
						fi
						i=$((i + 1))
					done
				else
					warn "unknown tool: $tok"
				fi
				;;
			*)
				if [ "$tok" -ge 1 ] && [ "$tok" -le "${#TOOLS[@]}" ]; then
					sel_toggle $((tok - 1))
				else
					warn "out of range: $tok"
				fi
				;;
			esac
		done
	done

	TARGETS=()
	if [ "${#SEL[@]}" -eq 0 ]; then
		info ""
		info "No tool selected."
		exit 0
	fi
	i=0
	while [ "$i" -lt "${#SEL[@]}" ]; do
		TARGETS[${#TARGETS[@]}]="${TOOLS[${SEL[$i]}]}"
		i=$((i + 1))
	done
	return 0
}

# ---------------------------------------------------------------- commands

# Turn the positional arguments into TARGETS (shared by install/uninstall).
resolve_names() {
	TARGETS=()
	local name
	for name in "$@"; do
		if [ "$name" = "all" ]; then
			TARGETS=("${TOOLS[@]}")
			return 0
		fi
		if ! known_tool "$name"; then
			die "unknown tool: $name (known: $FALLBACK_TOOLS)"
		fi
		TARGETS[${#TARGETS[@]}]="$name"
	done
}

cmd_install() {
	local n=0 failed=0

	if [ "$ALL" = 1 ]; then
		TARGETS=("${TOOLS[@]}")
	elif [ $# -gt 0 ]; then
		resolve_names "$@"
	else
		if ! selector; then
			die "no terminal to prompt on — pass the tools explicitly:
    install.sh install all
    install.sh install apkctl svg2vd"
		fi
	fi

	[ "${#TARGETS[@]}" -gt 0 ] || return 0
	printf '%sInstalling %d tool(s) → %s%s\n' \
		"$C_BOLD" "${#TARGETS[@]}" "$BIN_DIR" "$C_OFF"

	local t
	for t in ${TARGETS[@]+"${TARGETS[@]}"}; do
		if install_tool "$t"; then
			ok "$t ${LATEST_VERSION:-}"
			n=$((n + 1))
		else
			failed=$((failed + 1))
		fi
	done

	info ""
	path_note
	if [ "$failed" -gt 0 ]; then
		warn "$n installed, $failed failed"
		return 1
	fi
	info "Run 'install.sh update' any time to refresh them."
}

cmd_update() {
	installed_list
	local n=0 skipped=0 failed=0

	if [ "$ALL" = 1 ]; then
		TARGETS=("${INSTALLED[@]+"${INSTALLED[@]}"}")
	elif [ $# -gt 0 ]; then
		resolve_names "$@"
	else
		TARGETS=("${INSTALLED[@]+"${INSTALLED[@]}"}")
	fi

	if [ "${#TARGETS[@]}" -eq 0 ]; then
		info "Nothing installed yet — run 'install.sh' first."
		return 0
	fi

	printf '%sChecking %d installed tool(s) against the latest release%s\n' \
		"$C_BOLD" "${#TARGETS[@]}" "$C_OFF"

	local t
	for t in ${TARGETS[@]+"${TARGETS[@]}"}; do
		if ! is_installed "$t"; then
			warn "$t is not installed — skipped"
			failed=$((failed + 1))
			continue
		fi
		if needs_update "$t"; then
			if install_tool "$t"; then
				ok "$t → ${LATEST_VERSION:-latest}"
				n=$((n + 1))
			else
				failed=$((failed + 1))
			fi
		else
			info "  $t up to date"
			skipped=$((skipped + 1))
		fi
	done

	info ""
	info "$n updated, $skipped unchanged, $failed failed"
	[ "$failed" -eq 0 ]
}

cmd_list() {
	installed_list
	printf '%sandroid-tools%s' "$C_BOLD" "$C_OFF"
	if [ -n "$LATEST_VERSION" ]; then
		printf ' %s' "$LATEST_VERSION"
	fi
	printf ' — %s\n' "$BIN_DIR"

	local i=0 shown=0 t ver
	while [ "$i" -lt "${#TOOLS[@]}" ]; do
		t="${TOOLS[$i]}"
		if is_installed "$t"; then
			ver="$(installed_version "$t")"
			if [ -n "$LATEST_VERSION" ] && [ "$ver" != "unknown" ] && [ "$ver" != "$LATEST_VERSION" ]; then
				printf '  %-11s %-9s update available (%s → %s)\n' \
					"$t" "$ver" "$ver" "$LATEST_VERSION"
			else
				printf '  %-11s %-9s up to date\n' "$t" "$ver"
			fi
			shown=$((shown + 1))
		fi
		i=$((i + 1))
	done
	if [ "$shown" -eq 0 ]; then
		info "  nothing installed yet"
	fi

	info ""
	info "Available:"
	i=0
	while [ "$i" -lt "${#TOOLS[@]}" ]; do
		if ! is_installed "${TOOLS[$i]}"; then
			printf '  %-11s %s\n' "${TOOLS[$i]}" "${DESCS[$i]}"
		fi
		i=$((i + 1))
	done
}

cmd_uninstall() {
	if [ "$ALL" = 1 ]; then
		installed_list
		TARGETS=("${INSTALLED[@]+"${INSTALLED[@]}"}")
	elif [ $# -gt 0 ]; then
		resolve_names "$@"
	else
		die "name the tools to remove, or pass --all"
	fi

	if [ "${#TARGETS[@]}" -eq 0 ]; then
		info "Nothing installed."
		return 0
	fi

	local t
	for t in ${TARGETS[@]+"${TARGETS[@]}"}; do
		if [ -e "$BIN_DIR/$t" ]; then
			rm -f "$BIN_DIR/$t"
			drop_state "$t"
			ok "$t removed"
		else
			info "  $t was not installed"
		fi
	done
}

# -------------------------------------------------------------------- main

main() {
	local cmd="install"
	local -a rest=()
	while [ $# -gt 0 ]; do
		case "$1" in
		install | update | list | uninstall | help)
			cmd="$1"
			shift
			;;
		--update)
			cmd="update"
			shift
			;;
		--list)
			cmd="list"
			shift
			;;
		--uninstall)
			cmd="uninstall"
			shift
			;;
		--all)
			ALL=1
			shift
			;;
		--bin-dir)
			[ $# -ge 2 ] || die "--bin-dir needs a directory"
			BIN_DIR="$2"
			shift 2
			;;
		-h | --help)
			cmd="help"
			shift
			;;
		--)
			shift
			while [ $# -gt 0 ]; do
				rest[${#rest[@]}]="$1"
				shift
			done
			;;
		-*)
			die "unknown option: $1 (see install.sh help)"
			;;
		*)
			rest[${#rest[@]}]="$1"
			shift
			;;
		esac
	done

	case "$cmd" in
	help)
		usage
		return 0
		;;
	list)
		detect_target
		load_catalog
		load_state
		cmd_list
		return 0
		;;
	esac

	detect_target
	load_catalog
	load_state

	if [ "$(id -u)" = "0" ] && [ "$cmd" != "uninstall" ]; then
		warn "running as root — the tools will only be available to root"
	fi

	TMP_ROOT="$(mktemp -d 2>/dev/null || mktemp -d -t android-tools)"
	trap 'rm -rf "$TMP_ROOT"' EXIT

	case "$cmd" in
	install) cmd_install ${rest[@]+"${rest[@]}"} ;;
	update) cmd_update ${rest[@]+"${rest[@]}"} ;;
	uninstall) cmd_uninstall ${rest[@]+"${rest[@]}"} ;;
	esac
}

main "$@"
