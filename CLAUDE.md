# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
# Build release binary (outputs mcp-server/target/release/cta-mcp-server)
bash scripts/build.sh

# Run all tests (98 tests: unit + integration)
cargo test --all-targets --manifest-path mcp-server/Cargo.toml

# Run a single test by name
cargo test test_simple_session_e2e --manifest-path mcp-server/Cargo.toml

# Lint
cargo clippy --manifest-path mcp-server/Cargo.toml -- -D warnings

# Run CLI locally (for manual testing)
cargo run --manifest-path mcp-server/Cargo.toml --bin cta -- <command>
```

## Architecture

This is a Rust-based Claude Code plugin that parses JSONL session logs from `~/.claude/projects/` and provides token usage analytics via MCP tools and a CLI.

### Data Pipeline

```
~/.claude/projects/**/*.jsonl
    → parser.rs    : JSONL lines → ParseResult (zero-computation, deduplicates partial/final responses)
    → analyzer.rs  : ParseResult → SessionAnalysis (cost calculation via pricing.rs, 10-dimension metrics)
    → storage.rs   : SessionAnalysis → SQLite (upsert with change detection)
    → detector.rs  : DB queries → AnomalyReport (6 anomaly types, stddev-based thresholds)
    → bin/mcp.rs   : JSON-RPC over stdio (7 MCP tools via rmcp crate)
    → bin/cli.rs   : Human-readable CLI (9 commands via clap)
```

### Two Binaries

- **`cta-mcp-server`** (`src/bin/mcp.rs`) — MCP server; implements `ServerHandler` trait from `rmcp`, exposes 7 tools via `#[tool]` macro and `ToolRouter`
- **`cta`** (`src/bin/cli.rs`) — CLI for manual testing/debugging; mirrors MCP tool functionality

### Key Design Decisions

- **ParseResult is zero-computation**: `parser.rs` only extracts and deduplicates data from JSONL; all cost calculations happen in `analyzer.rs` using `PricingTable`
- **Pricing is embedded**: `config/pricing.toml` is read at runtime (or overridden via `CTA_PRICING_PATH`). Unknown models fall back to Sonnet-equivalent pricing under `[defaults]`
- **Three-mode path resolution** (`config.rs`): env var override > `$CLAUDE_PLUGIN_ROOT` plugin mode > `$HOME/.claude/` standalone mode. Projects dir has no plugin-mode override since Claude Code always writes sessions to `~/.claude/projects/`
- **Anomaly detection**: `detector.rs` uses standard deviation thresholds for 5 anomaly types, but `CostInefficient` uses absolute mean-based comparison (above-mean cost AND below-mean cache hit rate), making it independent of the `stddev_threshold` parameter

### Module Responsibilities

| Module | Role |
|--------|------|
| `parser.rs` | JSONL parsing, partial/final response deduplication, compression event detection |
| `analyzer.rs` | Cost breakdown, cache hit rate, model/tool ranking, session/project/global aggregation |
| `storage.rs` | SQLite schema, upsert pipeline, 24 query methods for all report types |
| `detector.rs` | 6-type statistical anomaly detection with severity scoring |
| `pricing.rs` | Model pricing lookup from TOML, cost calculation per token type |
| `archiver.rs` | zstd compression/decompression for session archival |
| `config.rs` | Centralized path resolution across three deployment modes |
| `session_finder.rs` | Recursive JSONL file discovery under projects directory |

### Plugin Structure

```
.claude-plugin/plugin.json  — Plugin manifest
.mcp.json                   — MCP server config (stdio transport via scripts/run.sh)
hooks/hooks.json            — SessionStart hook (auto-downloads binary on first run)
scripts/run.sh              — MCP server wrapper (ensures binary exists)
scripts/install.sh          — Binary installer (multi-platform, downloads from GitHub Releases)
skills/cta*/SKILL.md        — 6 workflow skills (router + 5 sub-skills)
```

## Commit Convention

`[type] Description` where type = feat/fix/refactor/docs/chore/test

## CI/CD

GitHub Actions (`.github/workflows/release.yml`) builds for 4 targets on tag push (`v*`):
macOS x86_64, macOS ARM64, Linux x86_64, Linux ARM64.
