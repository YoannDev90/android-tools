. "$(dirname "$0")/../utils/common.sh"
. "$(dirname "$0")/../utils/logging.sh"
. "$(dirname "$0")/../utils/hash.sh"
. "$(dirname "$0")/../utils/platform.sh"
. "$(dirname "$0")/../utils/http.sh"
. "$(dirname "$0")/../utils/catalog.sh"
. "$(dirname "$0")/../utils/tools.sh"
. "$(dirname "$0")/../utils/install.sh"
. "$(dirname "$0")/../utils/selector.sh"
. "$(dirname "$0")/../utils/commands.sh"

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
