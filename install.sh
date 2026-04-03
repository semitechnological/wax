#!/usr/bin/env bash
# wax installer — downloads the latest pre-built binary from GitHub Releases.
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/semitechnological/wax/master/install.sh | bash
#   # or with a specific version:
#   WAX_VERSION=v0.13.3 bash install.sh

set -euo pipefail

REPO="semitechnological/wax"
INSTALL_DIR="${WAX_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${WAX_VERSION:-}"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[0;33m'; CYAN='\033[0;36m'; BOLD='\033[1m'; NC='\033[0m'

info()  { printf "${CYAN}%s${NC}\n" "$*"; }
ok()    { printf "${GREEN}✓ %s${NC}\n" "$*"; }
warn()  { printf "${YELLOW}! %s${NC}\n" "$*" >&2; }
die()   { printf "${RED}error: %s${NC}\n" "$*" >&2; exit 1; }

# ---- detect OS / arch -------------------------------------------------------

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux)  os="linux" ;;
  Darwin) os="macos" ;;
  *)      die "Unsupported OS: $OS" ;;
esac

case "$ARCH" in
  x86_64|amd64)          arch="x64"   ;;
  aarch64|arm64)         arch="arm64" ;;
  *)                     die "Unsupported architecture: $ARCH" ;;
esac

ASSET="wax-${os}-${arch}"

# ---- resolve version --------------------------------------------------------

if [ -z "$VERSION" ]; then
  info "Fetching latest release version…"
  VERSION="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"\(.*\)".*/\1/')"
  [ -n "$VERSION" ] || die "Could not determine latest version from GitHub API"
fi

info "Installing wax ${VERSION} (${os}/${arch})…"

# ---- download ---------------------------------------------------------------

BASE_URL="https://github.com/${REPO}/releases/download/${VERSION}"
TMP="$(mktemp)"
TMP_SHA="$(mktemp)"
trap 'rm -f "$TMP" "$TMP_SHA"' EXIT

if command -v curl &>/dev/null; then
  curl -fsSL --progress-bar "${BASE_URL}/${ASSET}" -o "$TMP"
  # Fetch checksum file — releases before v0.13.3 don't have one; skip gracefully.
  HAVE_SHA=0
  if curl -fsSL "${BASE_URL}/${ASSET}.sha256" -o "$TMP_SHA" 2>/dev/null; then
    HAVE_SHA=1
  fi
elif command -v wget &>/dev/null; then
  wget -q --show-progress "${BASE_URL}/${ASSET}" -O "$TMP"
  HAVE_SHA=0
  if wget -q "${BASE_URL}/${ASSET}.sha256" -O "$TMP_SHA" 2>/dev/null; then
    HAVE_SHA=1
  fi
else
  die "curl or wget is required"
fi

# ---- verify checksum (when available) ---------------------------------------

if [ "$HAVE_SHA" -eq 1 ] && [ -s "$TMP_SHA" ]; then
  EXPECTED="$(tr -d '[:space:]' < "$TMP_SHA")"
  if command -v sha256sum &>/dev/null; then
    ACTUAL="$(sha256sum "$TMP" | awk '{print $1}')"
  elif command -v shasum &>/dev/null; then
    ACTUAL="$(shasum -a 256 "$TMP" | awk '{print $1}')"
  else
    warn "sha256sum/shasum not found — skipping integrity check"
    ACTUAL="$EXPECTED"
  fi

  [ "$ACTUAL" = "$EXPECTED" ] || die "SHA256 mismatch — download may be corrupted or tampered with
  expected: $EXPECTED
  actual:   $ACTUAL"
  ok "checksum verified"
else
  warn "No checksum file found for ${VERSION} — skipping integrity verification"
fi

chmod +x "$TMP"

# ---- install ----------------------------------------------------------------

mkdir -p "$INSTALL_DIR"
mv "$TMP" "${INSTALL_DIR}/wax"

ok "wax ${VERSION} installed to ${INSTALL_DIR}/wax"

# ---- PATH hint --------------------------------------------------------------

if ! command -v wax &>/dev/null 2>&1; then
  printf "\n${BOLD}Add wax to your PATH:${NC}\n"
  case "${SHELL:-}" in
    */fish) printf '  fish_add_path %s\n' "$INSTALL_DIR" ;;
    *)      printf '  echo '\''export PATH="%s:$PATH"'\'' >> ~/.bashrc  # or ~/.zshrc\n' "$INSTALL_DIR" ;;
  esac
fi
