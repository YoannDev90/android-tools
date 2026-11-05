#!/usr/bin/env bash
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
