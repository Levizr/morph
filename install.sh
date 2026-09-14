#!/bin/sh
set -eu

# Morph one-line installer (POSIX sh).
#
#   curl -fsSL https://morph.levizr.com/install.sh | sh
#
# Options (env vars):
#   MORPH_INSTALL_DIR=/path   install location (default: ~/.local/bin)
#   MORPH_VERSION=x.y.z       pin an exact version instead of latest

REPO="Levizr/morph"
INSTALL_DIR="${MORPH_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${MORPH_VERSION:-}"

# Detect OS
case "$(uname -s 2>/dev/null || printf unknown)" in
  Linux)            OS=linux ;;
  Darwin)           OS=macos ;;
  MINGW*|MSYS*|CYGWIN*) OS=windows ;;
  *)                OS=unknown ;;
esac

# Detect arch
case "$(uname -m 2>/dev/null || printf unknown)" in
  x86_64|amd64|Amd64)    ARCH=x64 ;;
  aarch64|arm64|AArch64) ARCH=arm64 ;;
  *)                     ARCH=unknown ;;
esac

# Platform label
case "$OS" in
  linux)   PLAT="Linux $ARCH" ;;
  macos)   PLAT="macOS $ARCH" ;;
  windows) PLAT="Windows $ARCH" ;;
  *)       PLAT="$OS $ARCH" ;;
esac

# Check platform is supported
case "$OS-$ARCH" in
  linux-x64|linux-arm64|macos-arm64|windows-x64) ;;
  *)
    printf '\nNo prebuilt morph binary for %s yet.\n\n' "$PLAT"
    printf 'Install with Cargo:\n'
    printf '    cargo install morphc\n\n'
    printf 'Or build from source:\n'
    printf '    git clone https://github.com/%s && cd morph\n' "$REPO"
    printf '    cargo install --path crates/morphc\n\n'
    exit 1
    ;;
esac

# Resolve version
if [ -z "$VERSION" ]; then
  VERSION="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null \
    | grep -oE '"tag_name"[[:space:]]*:[[:space:]]*"v[0-9]+\.[0-9]+\.[0-9]+"' \
    | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -n1)" || true
  [ -n "$VERSION" ] || { printf 'Error: could not determine latest version (set MORPH_VERSION to override)\n' >&2; exit 1; }
fi

if [ "$OS" = "windows" ]; then BIN_NAME="morph.exe"; else BIN_NAME="morph"; fi

# Intro
printf '\nMorph Installer v%s\n' "$VERSION"
printf '  Platform:   %s\n' "$PLAT"
printf '  Install to: %s/%s\n' "$INSTALL_DIR" "$BIN_NAME"
printf '\n'

# Download
URL="https://github.com/$REPO/releases/download/v$VERSION/morph-$OS-$ARCH.tar.gz"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

printf '[1/3] Downloading...\n'
printf '  %s\n' "$URL"

if command -v curl >/dev/null 2>&1; then
  curl --progress-bar --retry 3 -fSL "$URL" -o "$TMP/morph.tar.gz" \
    || { printf 'Error: download failed (release v%s may not exist for %s)\n' "$VERSION" "$PLAT" >&2; exit 1; }
else
  printf 'Error: curl is required but not found.\n' >&2
  exit 1
fi

printf '  %s bytes\n' "$(wc -c < "$TMP/morph.tar.gz" | tr -d ' ')"

# Extract
printf '\n[2/3] Extracting...\n'
tar -xzf "$TMP/morph.tar.gz" -C "$TMP"

# Checksum verification
if command -v sha256sum >/dev/null 2>&1; then
  GOT="$(sha256sum "$TMP/morph.tar.gz" | cut -d' ' -f1)"
elif command -v shasum >/dev/null 2>&1; then
  GOT="$(shasum -a 256 "$TMP/morph.tar.gz" | cut -d' ' -f1)"
else
  GOT=""
fi

if [ -n "$GOT" ] && [ -f "$TMP/manifest.json" ]; then
  EXPECT="$(grep -oE '"sha256"[[:space:]]*:[[:space:]]*"[a-f0-9]+"' "$TMP/manifest.json" \
    | grep -oE '[a-f0-9]{64}' | head -n1)" || true
  if [ -n "$EXPECT" ] && [ "$GOT" = "$EXPECT" ]; then
    printf '  Checksum verified\n'
  else
    printf '  Checksum mismatch (expected %s, got %s)\n' "$EXPECT" "$GOT"
  fi
else
  printf '  Extracted\n'
fi

# Install
BIN="$(find "$TMP" -type f -name "$BIN_NAME" | head -n1)"
[ -n "$BIN" ] || { printf 'Error: morph binary not found in archive\n' >&2; exit 1; }

printf '\n[3/3] Installing...\n'
mkdir -p "$INSTALL_DIR"

if command -v install >/dev/null 2>&1; then
  install -m 755 "$BIN" "$INSTALL_DIR/$BIN_NAME"
else
  cp "$BIN" "$INSTALL_DIR/$BIN_NAME"
  chmod 755 "$INSTALL_DIR/$BIN_NAME"
fi

printf '  Installed: %s/%s\n' "$INSTALL_DIR" "$BIN_NAME"

# PATH check
in_path=0
IFS=:
for p in $PATH; do [ "$p" = "$INSTALL_DIR" ] && in_path=1; done
unset IFS

if [ "$in_path" -eq 0 ]; then
  printf '\nNote: %s is not in your PATH.\n' "$INSTALL_DIR"
  case "${SHELL:-/bin/sh}" in
    *zsh)  printf '  echo '\''export PATH="$HOME/.local/bin:$PATH"'\'' >> ~/.zshrc && source ~/.zshrc\n' ;;
    *fish) printf '  fish_add_path ~/.local/bin\n' ;;
    *)     printf '  echo '\''export PATH="$HOME/.local/bin:$PATH"'\'' >> ~/.bashrc && source ~/.bashrc\n' ;;
  esac
fi

printf '\nDone. morph v%s installed successfully.\n\n' "$VERSION"
