#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CODEX_HOME="${CODEX_HOME:-$HOME/.codex}"
SKILLS_DIR="$CODEX_HOME/skills"

mkdir -p "$SKILLS_DIR"

SKILLS=(
    cta
    cta-health-check
    cta-cost-audit
    cta-anomaly-hunt
    cta-project-review
    cta-trend-watch
    cta-usage-pattern
)

for skill in "${SKILLS[@]}"; do
    target="$SKILLS_DIR/$skill"
    source="$REPO_ROOT/skills/$skill"

    if [ -L "$target" ] && [ "$(readlink "$target")" = "$source" ]; then
        continue
    fi

    if [ -e "$target" ] || [ -L "$target" ]; then
        echo "Refusing to overwrite existing Codex skill: $target" >&2
        echo "Remove or rename it first, then rerun this script." >&2
        exit 1
    fi

    ln -s "$source" "$target"
done

echo "Linked CTA skills into $SKILLS_DIR"
