#!/usr/bin/env bash
# Shared helpers for the android-tools installers.
log() { printf '%s\n' "$*"; }
err() { printf 'error: %s\n' "$*" >&2; }

have() { command -v "$1" >/dev/null 2>&1; }
