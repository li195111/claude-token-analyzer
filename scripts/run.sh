#!/usr/bin/env bash
set -euo pipefail

# Wrapper used by .mcp.json. The installer has a no-network cache fast path.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY="$(bash "$SCRIPT_DIR/install.sh")"

# Exec the binary (replaces this process)
exec "$BINARY" "$@"
