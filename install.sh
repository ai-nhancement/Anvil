#!/bin/sh
# Anvil installer (macOS / Linux).
#
# Downloads the latest prebuilt `anvil` binary for your OS + architecture and
# installs it onto your PATH. No Rust toolchain required.
#
#   curl -fsSL https://raw.githubusercontent.com/ai-nhancement/Anvil/master/install.sh | sh
#
# Overrides (environment variables):
#   ANVIL_VERSION   pin a specific release tag (e.g. v0.1.0); default: latest
#   ANVIL_BIN_DIR   install directory;            default: $HOME/.local/bin
set -eu

REPO="ai-nhancement/Anvil"
BIN="anvil"
BIN_DIR="${ANVIL_BIN_DIR:-$HOME/.local/bin}"

err()  { printf 'error: %s\n' "$1" >&2; exit 1; }
info() { printf '%s\n' "$1" >&2; }

# --- detect platform ---------------------------------------------------------
os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
  Linux)  os_part="unknown-linux-musl" ;;
  Darwin) os_part="apple-darwin" ;;
  *) err "unsupported OS: $os (on Windows use install.ps1)" ;;
esac

case "$arch" in
  x86_64 | amd64)  arch_part="x86_64" ;;
  arm64 | aarch64) arch_part="aarch64" ;;
  *) err "unsupported architecture: $arch" ;;
esac

target="${arch_part}-${os_part}"
asset="${BIN}-${target}.tar.gz"

# --- resolve download URL ----------------------------------------------------
# GitHub serves /releases/latest/download/<asset> as a redirect to the newest
# release, so we don't need the API or jq to find the latest version.
if [ "${ANVIL_VERSION:-}" = "" ]; then
  url="https://github.com/${REPO}/releases/latest/download/${asset}"
else
  url="https://github.com/${REPO}/releases/download/${ANVIL_VERSION}/${asset}"
fi

# --- download + extract ------------------------------------------------------
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

info "Downloading ${asset} ..."
if command -v curl >/dev/null 2>&1; then
  curl -fsSL "$url" -o "$tmp/$asset" || err "download failed: $url"
elif command -v wget >/dev/null 2>&1; then
  wget -qO "$tmp/$asset" "$url" || err "download failed: $url"
else
  err "need curl or wget to download"
fi

tar -xzf "$tmp/$asset" -C "$tmp" || err "failed to extract $asset"

# The binary may sit at the archive root or inside a folder, depending on layout.
bin_path="$(find "$tmp" -type f -name "$BIN" 2>/dev/null | head -n 1)"
[ -n "$bin_path" ] || err "binary '$BIN' not found in archive"

# --- install -----------------------------------------------------------------
mkdir -p "$BIN_DIR"
if command -v install >/dev/null 2>&1; then
  install -m 0755 "$bin_path" "$BIN_DIR/$BIN"
else
  cp "$bin_path" "$BIN_DIR/$BIN"
  chmod 0755 "$BIN_DIR/$BIN"
fi

info ""
info "Installed $BIN -> $BIN_DIR/$BIN"

# --- PATH hint ---------------------------------------------------------------
case ":$PATH:" in
  *":$BIN_DIR:"*)
    info "Run 'anvil init' in a repo, then 'anvil' to get started."
    ;;
  *)
    info ""
    info "NOTE: $BIN_DIR is not on your PATH. Add this line to your shell profile"
    info "(~/.bashrc, ~/.zshrc, ...), then restart your terminal:"
    info ""
    info "    export PATH=\"$BIN_DIR:\$PATH\""
    ;;
esac
