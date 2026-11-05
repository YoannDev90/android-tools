#!/usr/bin/env bash
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
