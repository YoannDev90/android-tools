#!/usr/bin/env bash
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
