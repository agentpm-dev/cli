#!/usr/bin/env bash
set -euo pipefail

OWNER="agentpm-dev"
REPO="cli"
BIN="agentpm"

os=$(uname -s)
arch=$(uname -m)

case "${os}-${arch}" in
  Linux-x86_64)   target="x86_64-unknown-linux-gnu" ;;
  Linux-aarch64)  target="aarch64-unknown-linux-gnu" ;;
  Darwin-x86_64)  target="x86_64-apple-darwin" ;;
  Darwin-arm64)   target="aarch64-apple-darwin" ;;
  *) echo "Unsupported OS/arch: ${os}/${arch}"; exit 1 ;;
esac

url="https://github.com/${OWNER}/${REPO}/releases/latest/download/${BIN}-${target}.tar.gz"
tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

echo "Downloading ${url}…"
curl -fsSL "$url" -o "${tmpdir}/${BIN}.tar.gz"

echo "Installing…"
tar -xzf "${tmpdir}/${BIN}.tar.gz" -C "${tmpdir}"

# Choose install dir
prefix="${PREFIX:-$HOME/.local}"
bindir="${prefix}/bin"
mkdir -p "$bindir"
install -m 0755 "${tmpdir}/${BIN}" "${bindir}/${BIN}"

case ":$PATH:" in
  *":${bindir}:"*) ;;
  *) echo "NOTE: add ${bindir} to your PATH (e.g., export PATH=\"${bindir}:\$PATH\")";;
esac

echo "Installed -> ${bindir}/${BIN}"
"${bindir}/${BIN}" --version || true
