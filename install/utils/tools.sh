#!/usr/bin/env bash
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
