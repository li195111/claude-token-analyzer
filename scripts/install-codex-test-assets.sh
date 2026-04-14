#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CODEX_HOME="${CODEX_HOME:-$HOME/.codex}"
CONFIG_FILE="$CODEX_HOME/config.toml"
RUNNER="$REPO_ROOT/scripts/run-codex.sh"

mkdir -p "$CODEX_HOME"

if [ ! -f "$CONFIG_FILE" ]; then
    touch "$CONFIG_FILE"
fi

if grep -q '^\[mcp_servers.token-analyzer\]$' "$CONFIG_FILE"; then
    if ! grep -q "^command = \"$RUNNER\"$" "$CONFIG_FILE"; then
        echo "Existing [mcp_servers.token-analyzer] entry found in $CONFIG_FILE with a different command." >&2
        echo "Update it manually to: $RUNNER" >&2
        exit 1
    fi
else
    cat <<EOF >> "$CONFIG_FILE"

[mcp_servers.token-analyzer]
command = "$RUNNER"
EOF
fi

bash "$SCRIPT_DIR/install-codex-cta-skills.sh"

echo "Installed Codex MCP config and CTA skills."
echo "Restart Codex, then use:"
echo "  \$cta 幫我看看狀況"
echo "  \$cta-usage-pattern 幫我分析這個 Claude session：<session_id>"
