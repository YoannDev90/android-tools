#!/usr/bin/env bash
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
