#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
INSTALLER="$REPO_ROOT/scripts/install.sh"
RUNNER="$REPO_ROOT/scripts/run.sh"
VERSION="0.3.0"
ASSET_NAME="cta-mcp-server-aarch64-apple-darwin"
TEST_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/cta-installer-test.XXXXXX")"
MOCK_BIN="$TEST_ROOT/mock-bin"
RELEASE_DIR="$TEST_ROOT/release"
DOWNLOAD_LOG="$TEST_ROOT/download.log"
MOCK_CURL_FAIL=0
MOCK_WGET_FAIL=0
export MOCK_CURL_FAIL MOCK_WGET_FAIL

cleanup() {
    rm -rf "$TEST_ROOT"
}
trap cleanup EXIT

mkdir -p "$MOCK_BIN" "$RELEASE_DIR"

cat > "$MOCK_BIN/uname" <<'EOF'
#!/usr/bin/env bash
case "${1:-}" in
    -s) printf '%s\n' "${MOCK_UNAME_S:-Darwin}" ;;
    -m) printf '%s\n' "${MOCK_UNAME_M:-arm64}" ;;
    *) exit 2 ;;
esac
EOF

cat > "$MOCK_BIN/curl" <<'EOF'
#!/usr/bin/env bash
url=""
destination=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        -o) destination="$2"; shift 2 ;;
        -*) shift ;;
        *) url="$1"; shift ;;
    esac
done
printf 'curl %s\n' "$url" >> "$MOCK_DOWNLOAD_LOG"
if [ "${MOCK_CURL_FAIL:-0}" = "1" ]; then
    exit 22
fi
source_file="$MOCK_RELEASE_DIR/${url##*/}"
[ -f "$source_file" ] || exit 22
cp "$source_file" "$destination"
EOF

cat > "$MOCK_BIN/wget" <<'EOF'
#!/usr/bin/env bash
url=""
destination=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        -O) destination="$2"; shift 2 ;;
        -*) shift ;;
        *) url="$1"; shift ;;
    esac
done
printf 'wget %s\n' "$url" >> "$MOCK_DOWNLOAD_LOG"
if [ "${MOCK_WGET_FAIL:-0}" = "1" ]; then
    exit 8
fi
source_file="$MOCK_RELEASE_DIR/${url##*/}"
[ -f "$source_file" ] || exit 8
cp "$source_file" "$destination"
EOF

chmod +x "$MOCK_BIN/uname" "$MOCK_BIN/curl" "$MOCK_BIN/wget"

cat > "$RELEASE_DIR/$ASSET_NAME" <<'EOF'
#!/usr/bin/env bash
printf 'fixture server: %s\n' "$*"
EOF
chmod +x "$RELEASE_DIR/$ASSET_NAME"

if command -v sha256sum >/dev/null 2>&1; then
    checksum_output="$(sha256sum "$RELEASE_DIR/$ASSET_NAME")"
else
    checksum_output="$(shasum -a 256 "$RELEASE_DIR/$ASSET_NAME")"
fi
checksum="${checksum_output%% *}"
printf '%s  %s\n' "$checksum" "$ASSET_NAME" > "$RELEASE_DIR/SHA256SUMS"

run_installer() {
    local plugin_data="$1"
    local stderr_file="$2"
    CLAUDE_PLUGIN_ROOT="$REPO_ROOT" \
    CLAUDE_PLUGIN_DATA="$plugin_data" \
    MOCK_DOWNLOAD_LOG="$DOWNLOAD_LOG" \
    MOCK_RELEASE_DIR="$RELEASE_DIR" \
    PATH="$MOCK_BIN:/usr/bin:/bin" \
    bash "$INSTALLER" 2>"$stderr_file"
}

assert_file_contains() {
    local path="$1"
    local expected="$2"
    if ! grep -Fq "$expected" "$path"; then
        printf 'expected %s to contain: %s\n' "$path" "$expected" >&2
        exit 1
    fi
}

fresh_data="$TEST_ROOT/fresh data"
fresh_stderr="$TEST_ROOT/fresh.stderr"
fresh_path="$(run_installer "$fresh_data" "$fresh_stderr")"
expected_path="$fresh_data/bin/cta-mcp-server-$VERSION"
[ "$fresh_path" = "$expected_path" ]
[ -x "$expected_path" ]
assert_file_contains "$DOWNLOAD_LOG" "releases/download/v$VERSION/$ASSET_NAME"
assert_file_contains "$DOWNLOAD_LOG" "releases/download/v$VERSION/SHA256SUMS"

: > "$DOWNLOAD_LOG"
cache_stderr="$TEST_ROOT/cache.stderr"
MOCK_CURL_FAIL=1
MOCK_WGET_FAIL=1
cache_path="$(run_installer "$fresh_data" "$cache_stderr")"
[ "$cache_path" = "$expected_path" ]
[ ! -s "$DOWNLOAD_LOG" ]
MOCK_CURL_FAIL=0
MOCK_WGET_FAIL=0

runner_output="$({
    CLAUDE_PLUGIN_ROOT="$REPO_ROOT" \
    CLAUDE_PLUGIN_DATA="$fresh_data" \
    MOCK_DOWNLOAD_LOG="$DOWNLOAD_LOG" \
    MOCK_RELEASE_DIR="$RELEASE_DIR" \
    PATH="$MOCK_BIN:/usr/bin:/bin" \
    bash "$RUNNER" alpha beta
} 2>"$TEST_ROOT/runner.stderr")"
[ "$runner_output" = "fixture server: alpha beta" ]

fallback_data="$TEST_ROOT/fallback"
fallback_stderr="$TEST_ROOT/fallback.stderr"
: > "$DOWNLOAD_LOG"
MOCK_CURL_FAIL=1
fallback_path="$(run_installer "$fallback_data" "$fallback_stderr")"
[ "$fallback_path" = "$fallback_data/bin/cta-mcp-server-$VERSION" ]
assert_file_contains "$DOWNLOAD_LOG" "curl https://github.com/li195111/claude-token-analyzer/releases/download/v$VERSION/$ASSET_NAME"
assert_file_contains "$DOWNLOAD_LOG" "wget https://github.com/li195111/claude-token-analyzer/releases/download/v$VERSION/$ASSET_NAME"
MOCK_CURL_FAIL=0

bad_release="$TEST_ROOT/bad-release"
mkdir -p "$bad_release"
cp "$RELEASE_DIR/$ASSET_NAME" "$bad_release/$ASSET_NAME"
printf '%064d  %s\n' 0 "$ASSET_NAME" > "$bad_release/SHA256SUMS"
bad_data="$TEST_ROOT/bad-data"
bad_path="$bad_data/bin/cta-mcp-server-$VERSION"
mkdir -p "$(dirname "$bad_path")"
printf 'preserve-me\n' > "$bad_path"
if CLAUDE_PLUGIN_ROOT="$REPO_ROOT" \
   CLAUDE_PLUGIN_DATA="$bad_data" \
   MOCK_DOWNLOAD_LOG="$DOWNLOAD_LOG" \
   MOCK_RELEASE_DIR="$bad_release" \
   PATH="$MOCK_BIN:/usr/bin:/bin" \
   bash "$INSTALLER" >"$TEST_ROOT/bad.stdout" 2>"$TEST_ROOT/bad.stderr"; then
    printf 'checksum mismatch unexpectedly succeeded\n' >&2
    exit 1
fi
[ "$(<"$bad_path")" = "preserve-me" ]

missing_release="$TEST_ROOT/missing-release"
mkdir -p "$missing_release"
if CLAUDE_PLUGIN_ROOT="$REPO_ROOT" \
   CLAUDE_PLUGIN_DATA="$TEST_ROOT/missing-data" \
   MOCK_DOWNLOAD_LOG="$DOWNLOAD_LOG" \
   MOCK_RELEASE_DIR="$missing_release" \
   PATH="$MOCK_BIN:/usr/bin:/bin" \
   bash "$INSTALLER" >"$TEST_ROOT/missing.stdout" 2>"$TEST_ROOT/missing.stderr"; then
    printf 'missing release unexpectedly succeeded\n' >&2
    exit 1
fi

: > "$DOWNLOAD_LOG"
if CLAUDE_PLUGIN_ROOT="$REPO_ROOT" \
   CLAUDE_PLUGIN_DATA="$TEST_ROOT/unsupported-data" \
   MOCK_UNAME_S="Windows_NT" \
   MOCK_DOWNLOAD_LOG="$DOWNLOAD_LOG" \
   MOCK_RELEASE_DIR="$RELEASE_DIR" \
   PATH="$MOCK_BIN:/usr/bin:/bin" \
   bash "$INSTALLER" >"$TEST_ROOT/unsupported.stdout" 2>"$TEST_ROOT/unsupported.stderr"; then
    printf 'unsupported platform unexpectedly succeeded\n' >&2
    exit 1
fi
[ ! -s "$DOWNLOAD_LOG" ]

printf 'installer integration: ok\n'
