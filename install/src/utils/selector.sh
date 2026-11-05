#!/usr/bin/env bash
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
