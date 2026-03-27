# Claude Token Analyzer

A Claude Code Plugin that analyzes session token usage, costs, trends, and anomalies.

## Overview

CTA provides 7 MCP tools and 6 workflow skills for understanding and optimizing Claude Code usage:

- **Token tracking** — Parse JSONL session logs into structured data
- **Cost analysis** — Per-session, per-project, and global cost breakdowns
- **Anomaly detection** — 6 statistical anomaly types with severity scoring
- **Trend forecasting** — Daily/weekly/monthly usage trends
- **Cache efficiency** — Cache hit rate analysis and optimization suggestions

## Installation

### From Marketplace (Recommended)

```bash
# Add marketplace
claude plugin marketplace add li195111/claude-token-analyzer

# Install plugin — binary auto-downloads on first session start
claude plugin install claude-token-analyzer
```

The MCP server binary is automatically downloaded from GitHub Releases on the first session via a `SessionStart` hook. No Rust toolchain required.

### From Source (Development)

```bash
git clone https://github.com/li195111/claude-token-analyzer.git
cd claude-token-analyzer
bash scripts/build.sh

# Launch with plugin loaded
claude --plugin-dir .
```

## Building from Source

```bash
bash scripts/build.sh
# Binary: mcp-server/target/release/cta-mcp-server
```

Requires: Rust toolchain (rustup.rs)

## Skills Reference

| Skill | Trigger | Purpose |
|-------|---------|---------|
| `cta` | "cta", "analyze tokens" | Router — routes to sub-skills |
| `cta-health-check` | "quick check", "overview" | One-page usage summary |
| `cta-cost-audit` | "monthly costs", "cost report" | Monthly cost breakdown |
| `cta-anomaly-hunt` | "anomalies", "problems" | Statistical anomaly scan |
| `cta-project-review` | "analyze project" | Four-dimension project analysis |
| `cta-trend-watch` | "trends", "burn rate" | Usage trend analysis |

## MCP Tools

| Tool | Parameters | Purpose |
|------|-----------|---------|
| `sync_db` | — | Sync JSONL session logs to SQLite |
| `analyze_session` | `session_id` | 10-dimension session analysis |
| `analyze_project` | `project_path`, `include_subagents`, `sort_by`, `limit` | Project aggregation |
| `analyze_global` | — | Cross-project panoramic view |
| `cost_report` | `month`, `daily`, `project_path` | Monthly cost report |
| `anomaly_scan` | `stddev_threshold`, `project_path`, `min_tokens_for_cache_check` | Anomaly detection |
| `trend_report` | `granularity`, `last_n_days`, `project_path` | Time-series trends |

## Data Flow

```
~/.claude/projects/**/*.jsonl
    → parser.rs (ParseResult)
    → analyzer.rs (SessionAnalysis)
    → storage.rs (SQLite DB)
    → MCP tools (JSON-RPC)
    → Skills (structured reports)
```

## Configuration

Environment variables (all optional):

| Variable | Purpose | Default |
|----------|---------|---------|
| `CTA_DB_PATH` | SQLite database location | `${CLAUDE_PLUGIN_ROOT}/data/token-analyzer.db` |
| `CTA_PROJECTS_DIR` | Session logs directory | `~/.claude/projects` |
| `CTA_ARCHIVE_DIR` | Archive directory | `~/.claude/token-analyzer-archive` |
| `CTA_PRICING_PATH` | Custom pricing TOML | Embedded in binary |

Path resolution priority: Environment variable > Plugin mode (`$CLAUDE_PLUGIN_ROOT`) > Standalone mode (`$HOME/.claude/`)

## Development

```bash
cd mcp-server

# Run all tests (98 tests)
cargo test --all-targets

# Lint
cargo clippy -- -D warnings

# Build release
cargo build --release
```

### Module Structure

```
mcp-server/src/
├── lib.rs              # Module declarations
├── types.rs            # Core data types (AssistantTurn, ParseResult, etc.)
├── parser.rs           # JSONL parsing + compression detection
├── pricing.rs          # Model pricing lookup + cost calculation
├── analyzer.rs         # 10-dimension session analysis
├── detector.rs         # 6-type anomaly detection with severity scoring
├── storage.rs          # SQLite persistence + sync pipeline
├── archiver.rs         # Session archival with zstd compression
├── config.rs           # Centralized path resolution (env/plugin/standalone)
├── format.rs           # Human-readable formatting helpers
├── session_finder.rs   # Recursive JSONL file search
└── bin/
    ├── mcp.rs          # MCP server binary (JSON-RPC over stdio)
    └── cli.rs          # CLI binary (cta command)
```

## License

MIT
