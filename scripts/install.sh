#!/usr/bin/env bash
set -euo pipefail

# CTA MCP Server installer
# Downloads and verifies the plugin-matched binary from GitHub Releases.

REPO="li195111/claude-token-analyzer"
BINARY_NAME="cta-mcp-server"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PLUGIN_ROOT="${CLAUDE_PLUGIN_ROOT:-$(cd "$SCRIPT_DIR/.." && pwd)}"
PLUGIN_MANIFEST="$PLUGIN_ROOT/.claude-plugin/plugin.json"

if [ ! -r "$PLUGIN_MANIFEST" ]; then
    printf 'Error: plugin manifest is not readable: %s\n' "$PLUGIN_MANIFEST" >&2
    exit 1
fi

VERSION="$(awk -F '"' '/^[[:space:]]*"version"[[:space:]]*:/ { print $4; exit }' "$PLUGIN_MANIFEST")"
if [ -z "$VERSION" ]; then
    printf 'Error: unable to read plugin version from %s\n' "$PLUGIN_MANIFEST" >&2
    exit 1
fi

# Determine install directory
if [ -n "${CLAUDE_PLUGIN_DATA:-}" ]; then
    INSTALL_DIR="$CLAUDE_PLUGIN_DATA/bin"
elif [ -n "${CLAUDE_PLUGIN_ROOT:-}" ]; then
    INSTALL_DIR="$CLAUDE_PLUGIN_ROOT/data/bin"
else
    INSTALL_DIR="${HOME}/.local/bin"
fi

INSTALL_PATH="$INSTALL_DIR/$BINARY_NAME-$VERSION"

# A versioned executable is its own cache identity.
if [ -x "$INSTALL_PATH" ]; then
    printf '%s\n' "$INSTALL_PATH"
    exit 0
fi

# Detect OS and architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Darwin) OS_TARGET="apple-darwin" ;;
    Linux)  OS_TARGET="unknown-linux-gnu" ;;
    *)      printf 'Unsupported OS: %s\n' "$OS" >&2; exit 1 ;;
esac

case "$ARCH" in
    x86_64)  ARCH_TARGET="x86_64" ;;
    arm64)   ARCH_TARGET="aarch64" ;;
    aarch64) ARCH_TARGET="aarch64" ;;
    *)       printf 'Unsupported architecture: %s\n' "$ARCH" >&2; exit 1 ;;
esac

ASSET_NAME="${BINARY_NAME}-${ARCH_TARGET}-${OS_TARGET}"
RELEASE_BASE_URL="https://github.com/$REPO/releases/download/v$VERSION"
ASSET_URL="$RELEASE_BASE_URL/$ASSET_NAME"
CHECKSUMS_URL="$RELEASE_BASE_URL/SHA256SUMS"

download_file() {
    local url="$1"
    local destination="$2"

    if command -v curl >/dev/null 2>&1; then
        if curl -fSL "$url" -o "$destination"; then
            return 0
        fi
        printf 'curl failed for %s; trying wget if available\n' "$url" >&2
    fi

    if command -v wget >/dev/null 2>&1; then
        if wget -q "$url" -O "$destination"; then
            return 0
        fi
    fi

    printf 'Error: failed to download %s with curl or wget\n' "$url" >&2
    exit 1
}

calculate_sha256() {
    local path="$1"
    local output

    if command -v sha256sum >/dev/null 2>&1; then
        output="$(sha256sum "$path")"
    elif command -v shasum >/dev/null 2>&1; then
        output="$(shasum -a 256 "$path")"
    else
        printf 'Error: sha256sum or shasum is required\n' >&2
        exit 1
    fi

    printf '%s\n' "${output%% *}"
}

mkdir -p "$INSTALL_DIR"
TEMP_BINARY="$(mktemp "$INSTALL_DIR/.${BINARY_NAME}.${VERSION}.binary.XXXXXX")"
TEMP_CHECKSUMS="$(mktemp "$INSTALL_DIR/.${BINARY_NAME}.${VERSION}.checksums.XXXXXX")"

cleanup() {
    rm -f "$TEMP_BINARY" "$TEMP_CHECKSUMS"
}
trap cleanup EXIT

printf 'Downloading CTA %s for %s-%s...\n' "$VERSION" "$ARCH_TARGET" "$OS_TARGET" >&2
download_file "$ASSET_URL" "$TEMP_BINARY"
download_file "$CHECKSUMS_URL" "$TEMP_CHECKSUMS"

EXPECTED_CHECKSUM="$(awk -v name="$ASSET_NAME" '
    {
        file = $2
        sub(/^\*/, "", file)
        if (file == name) {
            print $1
            exit
        }
    }
' "$TEMP_CHECKSUMS")"

if [ -z "$EXPECTED_CHECKSUM" ]; then
    printf 'Error: %s is not listed in SHA256SUMS\n' "$ASSET_NAME" >&2
    exit 1
fi

ACTUAL_CHECKSUM="$(calculate_sha256 "$TEMP_BINARY")"
if [ "$ACTUAL_CHECKSUM" != "$EXPECTED_CHECKSUM" ]; then
    printf 'Error: checksum mismatch for %s\n' "$ASSET_NAME" >&2
    exit 1
fi

chmod +x "$TEMP_BINARY"
mv -f "$TEMP_BINARY" "$INSTALL_PATH"
printf 'Installed CTA %s at %s\n' "$VERSION" "$INSTALL_PATH" >&2
printf '%s\n' "$INSTALL_PATH"
