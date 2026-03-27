#!/usr/bin/env bash
set -euo pipefail

# CTA MCP Server installer
# Downloads prebuilt binary from GitHub Releases to CLAUDE_PLUGIN_DATA

REPO="li195111/claude-token-analyzer"
BINARY_NAME="cta-mcp-server"

# Determine install directory
if [ -n "${CLAUDE_PLUGIN_DATA:-}" ]; then
    INSTALL_DIR="$CLAUDE_PLUGIN_DATA"
elif [ -n "${CLAUDE_PLUGIN_ROOT:-}" ]; then
    INSTALL_DIR="$CLAUDE_PLUGIN_ROOT/data"
else
    INSTALL_DIR="${HOME}/.local/bin"
fi

INSTALL_PATH="$INSTALL_DIR/$BINARY_NAME"

# Skip if binary already exists
if [ -f "$INSTALL_PATH" ]; then
    echo "Binary already installed: $INSTALL_PATH"
    exit 0
fi

# Detect OS and architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Darwin) OS_TARGET="apple-darwin" ;;
    Linux)  OS_TARGET="unknown-linux-gnu" ;;
    *)      echo "Unsupported OS: $OS"; exit 1 ;;
esac

case "$ARCH" in
    x86_64)  ARCH_TARGET="x86_64" ;;
    arm64)   ARCH_TARGET="aarch64" ;;
    aarch64) ARCH_TARGET="aarch64" ;;
    *)       echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

ASSET_NAME="${BINARY_NAME}-${ARCH_TARGET}-${OS_TARGET}"

# Get latest release URL
echo "Fetching latest release from $REPO..."
DOWNLOAD_URL="https://github.com/$REPO/releases/latest/download/$ASSET_NAME"

# Download
mkdir -p "$INSTALL_DIR"
echo "Downloading $ASSET_NAME to $INSTALL_PATH..."

if command -v curl >/dev/null 2>&1; then
    curl -fSL "$DOWNLOAD_URL" -o "$INSTALL_PATH"
elif command -v wget >/dev/null 2>&1; then
    wget -q "$DOWNLOAD_URL" -O "$INSTALL_PATH"
else
    echo "Error: curl or wget required"
    exit 1
fi

chmod +x "$INSTALL_PATH"
echo "Installed: $INSTALL_PATH"
