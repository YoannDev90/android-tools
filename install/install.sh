# Shared helpers for the android-tools installers.
log() { printf '%s\n' "$*"; }
err() { printf 'error: %s\n' "$*" >&2; }

have() { command -v "$1" >/dev/null 2>&1; }
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
# Shared SHA-256 helper.
sha256_file() {
	if have sha256sum; then
		sha256sum "$1" | awk '{print $1}'
	elif have shasum; then
		shasum -a 256 "$1" | awk '{print $1}'
	elif have openssl; then
		openssl dgst -sha256 "$1" | awk '{print $NF}'
	else
		return 1
	fi
}
# Shared platform detection.
detect_target() {
	local os arch
	case "$(uname -s)" in
	Linux) os="unknown-linux-musl" ;;
	Darwin) os="apple-darwin" ;;
	CYGWIN* | MSYS* | MINGW*)
		die "run install/install.ps1 on Windows"
		;;
	*) die "unsupported operating system: $(uname -s)" ;;
	esac
	case "$(uname -m)" in
	x86_64 | amd64) arch="x86_64" ;;
	aarch64 | arm64) arch="aarch64" ;;
	*) die "unsupported architecture: $(uname -m)" ;;
	esac
	TARGET="$arch-$os"
}
http_get() {
	if have curl; then
		curl --proto '=https' --tlsv1.2 -fsSL --retry 3 --retry-delay 1 "$1"
	elif have wget; then
		wget -qO- "$1"
	else
		die "curl or wget is required"
	fi
}

http_fetch() { # url dest
	if have curl; then
		curl --proto '=https' --tlsv1.2 -fsSL --retry 3 --retry-delay 1 -o "$2" "$1"
	elif have wget; then
		wget -qO "$2" "$1"
	else
		die "curl or wget is required"
	fi
}
parse_index() { # $1 = index.json contents
	local out
	if have python3; then
		out="$(printf '%s' "$1" | python3 -c '
import json, sys
d = json.load(sys.stdin)
print(d.get("version", ""))
for t in d.get("tools", []):
    if t.get("category") == "library":
        continue
    print("%s\t%s" % (t["id"], t.get("description", "").replace("\t", " ")))
' 2>/dev/null)" || return 1
	elif have jq; then
		out="$(printf '%s' "$1" | jq -r '.version as $v | $v, (.tools[] | select(.category != "library") | [.id, (.description // "")] | @tsv)')" || return 1
	else
		return 1
	fi
	[ -n "$out" ] || return 1
	LATEST_VERSION="$(printf '%s\n' "$out" | sed -n '1p')"
	while IFS=$'\t' read -r id desc; do
		[ -n "$id" ] || continue
		TOOLS[${#TOOLS[@]}]="$id"
		DESCS[${#DESCS[@]}]="$desc"
	done <<<"$(printf '%s\n' "$out" | sed '1d')"
	[ "${#TOOLS[@]}" -gt 0 ]
}

# Last-resort version: the /releases/latest redirect points straight at the tag,
# needs no API quota and no JSON parser.
version_from_redirect() {
	have curl || return 1
	curl --proto '=https' --tlsv1.2 -fsSIL -o /dev/null -w '%{url_effective}\n' \
		"https://github.com/$REPO/releases/latest" 2>/dev/null |
		sed -n 's|.*/tag/v\([0-9][0-9.]*\)$|\1|p'
}

load_catalog() {
	local raw=""
	raw="$(http_get "$INDEX_URL" 2>/dev/null || true)"
	if [ -n "$raw" ] && parse_index "$raw"; then
		return 0
	fi
	TOOLS=()
	DESCS=()
	local t
	for t in $FALLBACK_TOOLS; do
		TOOLS[${#TOOLS[@]}]="$t"
		DESCS[${#DESCS[@]}]=""
	done
	LATEST_VERSION="$(version_from_redirect || true)"
	warn "catalog unavailable — falling back to the built-in tool list"
}

# ------------------------------------------------------------------ state

load_state() {
	S_TOOL=()
	S_VER=()
	S_SHA=()
	[ -f "$STATE_FILE" ] || return 0
	local a b c
	while read -r a b c || [ -n "${a:-}" ]; do
		case "${a:-}" in
		"" | '#'*) continue ;;
		esac
		S_TOOL[${#S_TOOL[@]}]="$a"
		S_VER[${#S_VER[@]}]="${b:-}"
		S_SHA[${#S_SHA[@]}]="${c:-}"
	done <"$STATE_FILE"
}

state_index() { # tool -> prints its state row index, fails when untracked
	local i=0
	while [ "$i" -lt "${#S_TOOL[@]}" ]; do
		if [ "${S_TOOL[$i]}" = "$1" ]; then
			printf '%s' "$i"
			return 0
		fi
		i=$((i + 1))
	done
	return 1
}

save_state() {
	mkdir -p "$STATE_DIR"
	local i=0
	{
		printf '# android-tools installer state: <tool> <version> <sha256>\n'
		while [ "$i" -lt "${#S_TOOL[@]}" ]; do
			printf '%s %s %s\n' "${S_TOOL[$i]}" "${S_VER[$i]}" "${S_SHA[$i]}"
			i=$((i + 1))
		done
	} >"$STATE_FILE"
}

record_state() { # tool version sha256
	local idx
	idx="$(state_index "$1" || true)"
	if [ -n "$idx" ]; then
		S_VER[$idx]="$2"
		S_SHA[$idx]="$3"
	else
		S_TOOL[${#S_TOOL[@]}]="$1"
		S_VER[${#S_VER[@]}]="$2"
		S_SHA[${#S_SHA[@]}]="$3"
	fi
	save_state
}

drop_state() { # tool — forget one row, keeping the three arrays aligned
	local idx i=0
	idx="$(state_index "$1" || true)"
	[ -n "$idx" ] || return 0
	local new=() nv=() ns=()
	while [ "$i" -lt "${#S_TOOL[@]}" ]; do
		if [ "$i" != "$idx" ]; then
			new[${#new[@]}]="${S_TOOL[$i]}"
			nv[${#nv[@]}]="${S_VER[$i]}"
			ns[${#ns[@]}]="${S_SHA[$i]}"
		fi
		i=$((i + 1))
	done
	S_TOOL=("${new[@]+"${new[@]}"}")
	S_VER=("${nv[@]+"${nv[@]}"}")
	S_SHA=("${ns[@]+"${ns[@]}"}")
	save_state
}
is_installed() { [ -x "$BIN_DIR/$1" ]; }

known_tool() {
	local t
	for t in ${TOOLS[@]+"${TOOLS[@]}"}; do
		if [ "$t" = "$1" ]; then
			return 0
		fi
	done
	return 1
}

list_has() { # $1 in INSTALLED
	local t
	for t in ${INSTALLED[@]+"${INSTALLED[@]}"}; do
		if [ "$t" = "$1" ]; then
			return 0
		fi
	done
	return 1
}

installed_list() {
	INSTALLED=()
	local i=0 t
	while [ "$i" -lt "${#S_TOOL[@]}" ]; do
		t="${S_TOOL[$i]}"
		if is_installed "$t" && ! list_has "$t"; then
			INSTALLED[${#INSTALLED[@]}]="$t"
		fi
		i=$((i + 1))
	done
	for t in ${TOOLS[@]+"${TOOLS[@]}"}; do
		if is_installed "$t" && ! list_has "$t"; then
			INSTALLED[${#INSTALLED[@]}]="$t"
		fi
	done
}

installed_version() { # tool
	local idx ver=""
	idx="$(state_index "$1" || true)"
	if [ -n "$idx" ]; then
		ver="${S_VER[$idx]}"
	fi
	if [ -z "$ver" ] || [ "$ver" = "unknown" ]; then
		ver="$("$BIN_DIR/$1" --version 2>/dev/null | sed -n 's/.* \([0-9][0-9.]*\).*/\1/p' | sed -n '1p' || true)"
	fi
	printf '%s' "${ver:-unknown}"
}

# ------------------------------------------------------------ install/update
install_tool() { # tool — non-zero when the tool could not be installed
	local tool="$1"
	local asset="$tool-$TARGET.tar.xz"
	local url="$RELEASE_URL/$asset"
	local work="$TMP_ROOT/$tool"

	rm -rf "$work"
	mkdir -p "$work"

	if ! http_fetch "$url" "$work/$asset"; then
		warn "no release asset for $tool ($asset)"
		return 1
	fi

	local sha remote=""
	if sha="$(sha256_file "$work/$asset")"; then
		:
	else
		sha=""
	fi
	remote="$(http_get "$url.sha256" 2>/dev/null | awk '{print $1}' || true)"
	if [ -n "$remote" ] && [ -n "$sha" ] && [ "$remote" != "$sha" ]; then
		warn "checksum mismatch for $asset (expected $remote, got $sha)"
		return 1
	fi

	if ! tar -xJf "$work/$asset" -C "$work"; then
		warn "cannot extract $asset — is xz installed?"
		return 1
	fi

	local src=""
	if [ -f "$work/$tool-$TARGET/$tool" ]; then
		src="$work/$tool-$TARGET/$tool"
	else
		src="$(find "$work" -type f -name "$tool" 2>/dev/null | sed -n '1p')"
	fi
	if [ -z "$src" ] || [ ! -f "$src" ]; then
		warn "binary $tool not found inside $asset"
		return 1
	fi

	mkdir -p "$BIN_DIR"
	if have install; then
		install -m 0755 "$src" "$BIN_DIR/$tool"
	else
		cp "$src" "$BIN_DIR/$tool"
		chmod 0755 "$BIN_DIR/$tool"
	fi

	record_state "$tool" "${LATEST_VERSION:-unknown}" "$sha"

	if ! "$BIN_DIR/$tool" --version >/dev/null 2>&1; then
		warn "$tool is installed but '$tool --version' failed to run"
	fi

	# Best-effort: install the agent skill for this tool if available.
	local skill_url="https://raw.githubusercontent.com/$REPO/main/skills/$tool.md"
	local skill_tmp="$TMP_ROOT/$tool.skill.md"
	if http_fetch "$skill_url" "$skill_tmp" 2>/dev/null; then
		local agents_home="${AGENTS_HOME:-$HOME/.agents}"
		local canonical="$agents_home/skills/$tool"
		mkdir -p "$canonical"
		cp "$skill_tmp" "$canonical/SKILL.md" 2>/dev/null || true
		for agent in claude cursor codex gemini; do
			local agent_dir="$HOME/.$agent"
			[ -d "$agent_dir" ] || continue
			mkdir -p "$agent_dir/skills"
			local link="$agent_dir/skills/$tool"
			[ -e "$link" ] && rm -rf "$link"
			ln -s "$canonical" "$link" 2>/dev/null || true
		done
	fi
}

needs_update() { # tool
	local idx local_sha remote_sha local_ver
	idx="$(state_index "$1" || true)"
	if [ -z "$idx" ]; then
		# binary present but untracked (installed by hand or another
		# channel): refresh it so the next run has a baseline.
		return 0
	fi
	local_sha="${S_SHA[$idx]}"
	local_ver="${S_VER[$idx]}"
	remote_sha="$(http_get "$RELEASE_URL/$1-$TARGET.tar.xz.sha256" 2>/dev/null | awk '{print $1}' || true)"
	if [ -n "$remote_sha" ] && [ -n "$local_sha" ]; then
		if [ "$remote_sha" != "$local_sha" ]; then
			return 0
		fi
		return 1
	fi
	if [ -n "$local_ver" ] && [ -n "$LATEST_VERSION" ]; then
		if [ "$local_ver" != "$LATEST_VERSION" ]; then
			return 0
		fi
		return 1
	fi
	return 0 # cannot tell apart → refresh
}
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





# --------------------------------------------------------------- platform


# ---------------------------------------------------------------- catalog


# ------------------------------------------------------------- tool checks




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
