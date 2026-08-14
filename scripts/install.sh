#!/usr/bin/env bash
set -euo pipefail

# CTA MCP Server installer
# Downloads and verifies the plugin-matched binary from GitHub Releases.

REPO="li195111/claude-token-analyzer"
BINARY_NAME="cta-mcp-server"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PLUGIN_ROOT="${CLAUDE_PLUGIN_ROOT:-$(cd "$SCRIPT_DIR/.." && pwd)}"
PLUGIN_MANIFEST="$PLUGIN_ROOT/.claude-plugin/plugin.json"

# Local development override: run an explicit prebuilt binary instead of the
# release download. Documented in README "Building from Source".
if [ -n "${CTA_LOCAL_BINARY:-}" ]; then
    if [ -f "$CTA_LOCAL_BINARY" ] && [ -x "$CTA_LOCAL_BINARY" ]; then
        printf '%s\n' "$CTA_LOCAL_BINARY"
        exit 0
    fi
    printf 'Error: CTA_LOCAL_BINARY is not an executable file: %s\n' "$CTA_LOCAL_BINARY" >&2
    exit 1
fi

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
CHECKSUM_PATH="$INSTALL_PATH.sha256"

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

# A cached binary is reused only when it is a regular executable file that
# re-verifies against the checksum recorded at install time; anything else
# falls through to a fresh install.
if [ -f "$INSTALL_PATH" ] && [ -x "$INSTALL_PATH" ] && [ -r "$CHECKSUM_PATH" ]; then
    RECORDED_CHECKSUM="$(cat "$CHECKSUM_PATH")"
    if [ -n "$RECORDED_CHECKSUM" ] && [ "$(calculate_sha256 "$INSTALL_PATH")" = "$RECORDED_CHECKSUM" ]; then
        printf '%s\n' "$INSTALL_PATH"
        exit 0
    fi
    printf 'Cached binary failed checksum verification; reinstalling %s\n' "$INSTALL_PATH" >&2
elif [ -e "$INSTALL_PATH" ] && [ ! -f "$INSTALL_PATH" ]; then
    # A directory here would swallow the later mv -f instead of failing it.
    printf 'Cached entry is not a regular file; removing %s before reinstall\n' "$INSTALL_PATH" >&2
    rm -rf -- "$INSTALL_PATH" "$CHECKSUM_PATH"
elif [ -e "$INSTALL_PATH" ]; then
    printf 'Cached entry is not a verified regular executable; reinstalling %s\n' "$INSTALL_PATH" >&2
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
        if curl -fSL --connect-timeout 10 --max-time 300 --retry 2 "$url" -o "$destination"; then
            return 0
        fi
        printf 'curl failed for %s; trying wget if available\n' "$url" >&2
    fi

    if command -v wget >/dev/null 2>&1; then
        if wget -q --timeout=30 --tries=3 "$url" -O "$destination"; then
            return 0
        fi
    fi

    printf 'Error: failed to download %s with curl or wget\n' "$url" >&2
    exit 1
}

mkdir -p "$INSTALL_DIR"

TEMP_BINARY=""
TEMP_CHECKSUMS=""
TEMP_RECORD=""

cleanup() {
    if [ -n "$TEMP_BINARY" ]; then rm -f "$TEMP_BINARY"; fi
    if [ -n "$TEMP_CHECKSUMS" ]; then rm -f "$TEMP_CHECKSUMS"; fi
    if [ -n "$TEMP_RECORD" ]; then rm -f "$TEMP_RECORD"; fi
}
trap cleanup EXIT

# Sweep temp files leaked by an interrupted install. The age filter avoids
# racing a concurrent in-progress install in the same directory.
find "$INSTALL_DIR" -maxdepth 1 -name ".${BINARY_NAME}.*" -mmin +60 -exec rm -f {} + || true

TEMP_BINARY="$(mktemp "$INSTALL_DIR/.${BINARY_NAME}.${VERSION}.binary.XXXXXX")"
TEMP_CHECKSUMS="$(mktemp "$INSTALL_DIR/.${BINARY_NAME}.${VERSION}.checksums.XXXXXX")"
TEMP_RECORD="$(mktemp "$INSTALL_DIR/.${BINARY_NAME}.${VERSION}.record.XXXXXX")"

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
# The checksum record lands last so its presence certifies a completed install.
printf '%s\n' "$EXPECTED_CHECKSUM" > "$TEMP_RECORD"
mv -f "$TEMP_RECORD" "$CHECKSUM_PATH"
printf 'Installed CTA %s at %s\n' "$VERSION" "$INSTALL_PATH" >&2
printf '%s\n' "$INSTALL_PATH"
