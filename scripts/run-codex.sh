#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TEST_ROOT="$REPO_ROOT/.codex-test"

if [ -z "${CTA_DB_PATH:-}" ]; then
    export CTA_DB_PATH="$TEST_ROOT/token-analyzer.db"
fi

if [ -z "${CTA_ARCHIVE_DIR:-}" ]; then
    export CTA_ARCHIVE_DIR="$TEST_ROOT/token-analyzer-archive"
fi

if [ -z "${CTA_PROJECTS_DIR:-}" ]; then
    if [ -n "${CLAUDE_CONFIG_DIR:-}" ]; then
        export CTA_PROJECTS_DIR="$CLAUDE_CONFIG_DIR/projects"
    else
        export CTA_PROJECTS_DIR="$HOME/.claude/projects"
    fi
fi

mkdir -p "$TEST_ROOT"
mkdir -p "$CTA_ARCHIVE_DIR"

if [ -x "$REPO_ROOT/mcp-server/target/release/cta-mcp-server" ]; then
    BINARY="$REPO_ROOT/mcp-server/target/release/cta-mcp-server"
elif [ -x "$REPO_ROOT/mcp-server/target/debug/cta-mcp-server" ]; then
    BINARY="$REPO_ROOT/mcp-server/target/debug/cta-mcp-server"
else
    echo "cta-mcp-server not found. Run 'bash $REPO_ROOT/scripts/build.sh' or 'cargo build --manifest-path $REPO_ROOT/mcp-server/Cargo.toml --bin cta-mcp-server' first." >&2
    exit 1
fi

exec "$BINARY" "$@"
