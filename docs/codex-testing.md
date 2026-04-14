# Codex MCP + Skill Testing

This guide mounts the local `claude-token-analyzer` repo into Codex so Codex can:

- call the repo's MCP server directly
- use the repo's CTA skills natively
- analyze local Claude JSONL session logs without relying on Claude plugin runtime

## 1. Build the MCP server

```bash
bash scripts/build.sh
```

This produces:

```text
mcp-server/target/release/cta-mcp-server
```

## 2. Fast path: one-shot install

Run:

```bash
bash scripts/install-codex-test-assets.sh
```

This script:

- appends a `token-analyzer` MCP server entry to `~/.codex/config.toml`
- links the CTA skills into `~/.codex/skills/`
- refuses to overwrite conflicting existing CTA skill directories

If you prefer to update Codex config manually, use the next section instead.

## 3. Mount the MCP server into Codex manually

Add this block to `~/.codex/config.toml`:

```toml
[mcp_servers.token-analyzer]
command = "/Users/liyuefong/Desktop/claude-token-analyzer/scripts/run-codex.sh"
```

### What `run-codex.sh` does

- prefers `mcp-server/target/release/cta-mcp-server`
- falls back to `mcp-server/target/debug/cta-mcp-server`
- defaults test DB to `.codex-test/token-analyzer.db`
- defaults archive dir to `.codex-test/token-analyzer-archive`
- defaults projects dir to:
  - `CTA_PROJECTS_DIR` if already set
  - otherwise `CLAUDE_CONFIG_DIR/projects`
  - otherwise `~/.claude/projects`

This keeps Codex smoke tests isolated from plugin DB/archive state.

## 4. Link repo CTA skills into Codex manually

Run:

```bash
bash scripts/install-codex-cta-skills.sh
```

The script creates symlinks in `~/.codex/skills/` for:

- `cta`
- `cta-health-check`
- `cta-cost-audit`
- `cta-anomaly-hunt`
- `cta-project-review`
- `cta-trend-watch`
- `cta-usage-pattern`

The script refuses to overwrite existing skill directories or symlinks.

## 5. Restart Codex

Start a new Codex session in this repo after the MCP config and skill symlinks are in place.

## 6. MCP smoke tests inside Codex

Use prompts that force direct tool invocation:

```text
請直接呼叫 token-analyzer 的 sync_db
```

```text
請直接呼叫 token-analyzer 的 classify_session_pattern，session_id=<完整 session_id>
```

```text
請直接呼叫 token-analyzer 的 classify_session_pattern，session_id=<唯一前綴>
```

```text
請直接測試一個不存在的 session_id，確認回傳 SESSION_NOT_FOUND
```

Expected checks:

- `sync_db` succeeds
- exact `session_id` lookup succeeds
- unique prefix lookup succeeds
- symbolic business error is exposed in `error.data.code`

## 7. Skill smoke tests inside Codex

After the symlinks are installed, test native skill routing with these prompts:

### Router

```text
$cta 幫我看看狀況
```

### Usage pattern

```text
$cta-usage-pattern 幫我分析這個 Claude session：<session_id>
```

```text
$cta-usage-pattern 幫我判斷這個 session 是不是 correction_spiral，並給我 harness 優化建議：<session_id>
```

```text
$cta-usage-pattern 先分析 session 模式，再補 14d token sparkline：<session_id>
```

Expected checks:

- Codex recognizes the CTA skills without manually opening repo files first
- `cta-usage-pattern` calls `classify_session_pattern`
- output includes `pattern`, `severity`, signal summary, and 2-4 workflow adjustments
- ambiguous prefix causes the skill to ask for a longer id instead of guessing

## 8. Fast local verification before Codex mount

These repo tests should already be green before debugging Codex integration:

```bash
cargo test --all-targets --manifest-path mcp-server/Cargo.toml
cargo clippy --all-targets --manifest-path mcp-server/Cargo.toml -- -D warnings
```

## 9. Troubleshooting

### Codex cannot start the MCP server

Check that:

- `bash scripts/build.sh` completed successfully
- `scripts/run-codex.sh` exists and is executable
- `~/.codex/config.toml` points to the repo's absolute path

### Skill triggers but cannot find the tool reference

The CTA skills were refactored to use relative reference paths. If Codex still reads an old copy, remove the stale skill under `~/.codex/skills/` and re-run:

```bash
bash scripts/install-codex-cta-skills.sh
```

### You need to point Codex at a different Claude log root

Launch Codex with `CTA_PROJECTS_DIR` or `CLAUDE_CONFIG_DIR` set in the environment before startup, or adjust `scripts/run-codex.sh`.
