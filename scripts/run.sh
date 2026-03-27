#!/usr/bin/env bash
set -euo pipefail

# Wrapper: ensure binary exists, then exec it
# Used by .mcp.json as the MCP server command

BINARY_NAME="cta-mcp-server"

# Determine binary location
if [ -n "${CLAUDE_PLUGIN_DATA:-}" ]; then
    BINARY="$CLAUDE_PLUGIN_DATA/$BINARY_NAME"
elif [ -n "${CLAUDE_PLUGIN_ROOT:-}" ]; then
    BINARY="$CLAUDE_PLUGIN_ROOT/data/$BINARY_NAME"
else
    BINARY="${HOME}/.local/bin/$BINARY_NAME"
fi

# If binary missing, download it
if [ ! -f "$BINARY" ]; then
    SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
    bash "$SCRIPT_DIR/install.sh" >&2
fi

# Exec the binary (replaces this process)
exec "$BINARY" "$@"
