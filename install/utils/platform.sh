#!/usr/bin/env bash
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
