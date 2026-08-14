#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
INSTALLER="$REPO_ROOT/scripts/install.sh"
RUNNER="$REPO_ROOT/scripts/run.sh"
VERSION="$(awk -F '"' '/^[[:space:]]*"version"[[:space:]]*:/ { print $4; exit }' "$REPO_ROOT/.claude-plugin/plugin.json")"
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
        --connect-timeout|--max-time|--retry) shift 2 ;;
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
[ -r "$expected_path.sha256" ]
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

# A corrupted cached binary must fail re-verification and be reinstalled.
corrupt_stderr="$TEST_ROOT/corrupt.stderr"
: > "$DOWNLOAD_LOG"
printf 'corrupted-binary\n' > "$expected_path"
chmod +x "$expected_path"
corrupt_path="$(run_installer "$fresh_data" "$corrupt_stderr")"
[ "$corrupt_path" = "$expected_path" ]
assert_file_contains "$corrupt_stderr" "failed checksum verification"
assert_file_contains "$DOWNLOAD_LOG" "releases/download/v$VERSION/$ASSET_NAME"
cmp -s "$expected_path" "$RELEASE_DIR/$ASSET_NAME"

# A cached binary without its checksum record is untrusted and reinstalled.
missing_record_stderr="$TEST_ROOT/missing-record.stderr"
: > "$DOWNLOAD_LOG"
rm -f "$expected_path.sha256"
missing_record_path="$(run_installer "$fresh_data" "$missing_record_stderr")"
[ "$missing_record_path" = "$expected_path" ]
assert_file_contains "$missing_record_stderr" "not a verified regular executable"
assert_file_contains "$DOWNLOAD_LOG" "releases/download/v$VERSION/$ASSET_NAME"
[ -r "$expected_path.sha256" ]

# A directory occupying the cache path is removed and replaced by a fresh install.
dir_data="$TEST_ROOT/dir-data"
dir_path="$dir_data/bin/cta-mcp-server-$VERSION"
mkdir -p "$dir_path"
: > "$DOWNLOAD_LOG"
dir_stderr="$TEST_ROOT/dir.stderr"
dir_result="$(run_installer "$dir_data" "$dir_stderr")"
[ "$dir_result" = "$dir_path" ]
[ -f "$dir_path" ]
cmp -s "$dir_path" "$RELEASE_DIR/$ASSET_NAME"
assert_file_contains "$dir_stderr" "not a regular file"

# CTA_LOCAL_BINARY overrides the download path entirely for local development.
local_bin="$TEST_ROOT/local dev/cta-local"
mkdir -p "$(dirname "$local_bin")"
printf '#!/usr/bin/env bash\nprintf "local dev binary\\n"\n' > "$local_bin"
chmod +x "$local_bin"
: > "$DOWNLOAD_LOG"
override_path="$({
    CTA_LOCAL_BINARY="$local_bin" \
    CLAUDE_PLUGIN_ROOT="$REPO_ROOT" \
    CLAUDE_PLUGIN_DATA="$TEST_ROOT/override-data" \
    MOCK_DOWNLOAD_LOG="$DOWNLOAD_LOG" \
    MOCK_RELEASE_DIR="$RELEASE_DIR" \
    PATH="$MOCK_BIN:/usr/bin:/bin" \
    bash "$INSTALLER"
} 2>"$TEST_ROOT/override.stderr")"
[ "$override_path" = "$local_bin" ]
[ ! -s "$DOWNLOAD_LOG" ]

# A CTA_LOCAL_BINARY that is not an executable file fails fast.
if CTA_LOCAL_BINARY="$TEST_ROOT/does-not-exist" \
   CLAUDE_PLUGIN_ROOT="$REPO_ROOT" \
   CLAUDE_PLUGIN_DATA="$TEST_ROOT/override-data" \
   MOCK_DOWNLOAD_LOG="$DOWNLOAD_LOG" \
   MOCK_RELEASE_DIR="$RELEASE_DIR" \
   PATH="$MOCK_BIN:/usr/bin:/bin" \
   bash "$INSTALLER" >"$TEST_ROOT/override-bad.stdout" 2>"$TEST_ROOT/override-bad.stderr"; then
    printf 'invalid CTA_LOCAL_BINARY unexpectedly succeeded\n' >&2
    exit 1
fi
assert_file_contains "$TEST_ROOT/override-bad.stderr" "CTA_LOCAL_BINARY is not an executable file"

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
