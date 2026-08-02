# Claude Token Analyzer

> Your Claude Code sessions might be burning tokens you can't see.
> **Diagnoses** where your tokens go, why they're wasted, and what to fix first.

**Fully local** — parses your `~/.claude` JSONL files into SQLite. Nothing leaves your machine. No cloud. No telemetry.

[![MIT License](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![GitHub Release](https://img.shields.io/github/v/release/li195111/claude-token-analyzer)](https://github.com/li195111/claude-token-analyzer/releases)

[English](#features) | [繁體中文](#繁體中文)

## Features

- **Diagnose token waste** — Detects 6 statistical anomaly types (HighCost, LowCacheHitRate, CostInefficient, ExcessiveToolUse, HighTokenUsage, UnusualModelMix) with severity scoring
- **Audit costs** — Per-session, per-project, and global cost breakdowns with monthly trends
- **Forecast spending** — Daily/weekly/monthly usage trends with burn-rate projections
- **Optimize cache** — Identifies sessions with poor cache hit rates that inflate costs
- **Prioritize fixes** — Severity-scored anomalies so you know what to fix first, not just what's wrong
- **Converse naturally** — Ask in plain language: "how much did I spend?" or "scan for anomalies"

## Quick Start

```bash
# Add this repository as a marketplace, then install the qualified plugin
claude plugin marketplace add https://github.com/li195111/claude-token-analyzer.git
claude plugin install claude-token-analyzer@claude-token-analyzer
```

The first MCP launch downloads the binary matching the plugin version and verifies it against that release's `SHA256SUMS`. No Rust toolchain is required.

Supported platforms: macOS x86_64, macOS arm64, Linux x86_64, and Linux arm64.

Then just ask in any Claude Code session:

```
> cta
> how much did I spend this month?
> scan for anomalies
> analyze this project
> show me usage trends
```

## How It Works

```
~/.claude/projects/**/*.jsonl       Your session logs (never modified)
    → parser.rs                     Extract + deduplicate responses
    → analyzer.rs                   Cost calculation, 10-dimension metrics
    → storage.rs                    Upsert into local SQLite
    → detector.rs                   6-type anomaly detection + severity scoring
    → MCP tools / Skills            You ask, it answers
```

All processing happens locally. The SQLite database lives under the plugin data directory in plugin mode and falls back to `~/.claude/` in standalone mode. No network calls, no external dependencies at runtime.

## Skills

| Skill | Trigger Phrases | What It Does |
|-------|----------------|--------------|
| `cta` | "cta", "analyze tokens" | Routes to the right sub-skill |
| `cta-health-check` | "quick check", "overview", "看看狀況" | One-page usage summary |
| `cta-cost-audit` | "monthly costs", "cost report", "這個月花多少" | Monthly cost breakdown with model split |
| `cta-anomaly-hunt` | "anomalies", "problems", "有異常嗎" | Statistical anomaly scan with drill-down |
| `cta-project-review` | "analyze project", "專案健檢" | Four-dimension project analysis |
| `cta-trend-watch` | "trends", "burn rate", "趨勢" | Usage trend analysis with forecasting |
| `cta-usage-pattern` | "usage pattern", "workflow advice", "使用模式" | Session-pattern diagnosis and harness guidance |

## MCP Tools

| Tool | Purpose |
|------|---------|
| `sync_db` | Sync JSONL session logs to SQLite |
| `analyze_session` | 10-dimension session analysis |
| `classify_session_pattern` | Classify one session from hard workflow signals |
| `analyze_project` | Project-level aggregation with sorting |
| `analyze_global` | Cross-project panoramic view |
| `cost_report` | Monthly cost report (daily granularity available) |
| `anomaly_scan` | 6-type anomaly detection with severity scoring |
| `trend_report` | Time-series trends (daily/weekly/monthly) |

## Configuration

### Output language

CTA uses Claude Code's native plugin configuration. Human-readable reports default to `auto`, which follows the latest user message's primary language and falls back to English when unclear. Set a language during a fresh install with:

```bash
claude plugin install claude-token-analyzer@claude-token-analyzer --config output_language=en
```

Supported values are `auto`, `en`, and `zh-TW`. Empty, unsupported, or unexpanded values safely behave as `auto`; metric names, tool names, pattern IDs, JSON fields, and session IDs remain English. Existing users can run `/plugin configure` in Claude Code and select `output_language`.

This configuration flow is verified with Claude Code 2.1.220. If your client does not recognize `--config` or `/plugin configure`, upgrade Claude Code before configuring CTA.

### Environment variables

Environment variables (all optional):

| Variable | Purpose | Default |
|----------|---------|---------|
| `CTA_DB_PATH` | SQLite database location | `${CLAUDE_PLUGIN_ROOT}/data/token-analyzer.db` or `~/.claude/token-analyzer.db` |
| `CTA_PROJECTS_DIR` | Session logs directory | `${CLAUDE_CONFIG_DIR}/projects` or `~/.claude/projects` |
| `CTA_ARCHIVE_DIR` | Archive directory | `${CLAUDE_PLUGIN_ROOT}/data/token-analyzer-archive` or `~/.claude/token-analyzer-archive` |
| `CTA_PRICING_PATH` | Custom pricing TOML | Embedded in binary |
| `CLAUDE_CONFIG_DIR` | Claude config root for session logs | unset |

Path resolution priority:
- DB/archive: environment variable > plugin mode (`$CLAUDE_PLUGIN_ROOT`) > standalone mode (`$HOME/.claude/`)
- Projects: environment variable > Claude config dir (`$CLAUDE_CONFIG_DIR/projects`) > standalone mode (`$HOME/.claude/projects`)

## Building from Source

```bash
git clone https://github.com/li195111/claude-token-analyzer.git
cd claude-token-analyzer
bash scripts/build.sh
# Binary: mcp-server/target/release/cta-mcp-server

# Run 173+ automated tests
cargo test --all-targets --locked --manifest-path mcp-server/Cargo.toml

# Lint
cargo clippy --all-targets --locked --manifest-path mcp-server/Cargo.toml -- -D warnings

# Launch with plugin loaded
claude --plugin-dir .
```

Requires: [Rust toolchain](https://rustup.rs)

## Contributing

Issues and PRs welcome! See [open issues](https://github.com/li195111/claude-token-analyzer/issues) for starter tasks.

**Development setup:**
1. Clone the repo and run `bash scripts/build.sh`
2. Run `cargo test --all-targets --locked --manifest-path mcp-server/Cargo.toml` to verify
3. Load the plugin locally with `claude --plugin-dir .`

Rust toolchain required. The project uses `cargo clippy -- -D warnings` for linting.

## License

MIT

---

## 繁體中文

> 你的 Claude Code 會話可能正在浪費你看不見的 token。
> **診斷** token 流向、浪費原因，並告訴你該優先修正什麼。

**全本地運行** — 解析 `~/.claude` JSONL 檔案到 SQLite。資料不離開你的機器。無雲端、無遙測。

### 功能特色

- **診斷 token 浪費** — 6 種統計異常類型，含嚴重度評分
- **成本審計** — 按會話、專案、全域的費用拆解與月度趨勢
- **趨勢預測** — 日/週/月用量趨勢與燃燒率預測
- **快取優化** — 識別低快取命中率的會話，降低不必要開銷
- **嚴重度排序** — 優先處理影響最大的問題，不只是標記異常
- **自然語言互動** — 用中文直接問：「看看狀況」「這個月花多少」「有異常嗎」

### 快速開始

```bash
# 加入 Marketplace，再用完整 plugin id 安裝
claude plugin marketplace add https://github.com/li195111/claude-token-analyzer.git
claude plugin install claude-token-analyzer@claude-token-analyzer
```

第一次啟動 MCP 時會下載與 plugin 版本一致的 binary，並使用同一個 release 的 `SHA256SUMS` 驗證。無需安裝 Rust 工具鏈。

支援平台：macOS x86_64、macOS arm64、Linux x86_64、Linux arm64。

輸出語系預設為 `auto`，會跟隨使用者最新訊息的主要語言。新安裝可加上 `--config output_language=zh-TW`；既有使用者可在 Claude Code 執行 `/plugin configure`。可用值為 `auto`、`en`、`zh-TW`。

然後在 Claude Code 中直接問：

```
> 看看狀況
> 這個月花多少？
> 有異常嗎？
> 分析這個專案
> 用量趨勢
```

### 技能一覽

| 技能 | 觸發語 | 功能 |
|------|--------|------|
| `cta` | "cta"、"分析 token" | 智能路由至子技能 |
| `cta-health-check` | "看看狀況"、"總覽" | 一頁式用量摘要 |
| `cta-cost-audit` | "這個月花多少"、"成本報告" | 月度費用明細 |
| `cta-anomaly-hunt` | "有異常嗎"、"排查" | 統計異常掃描 |
| `cta-project-review` | "專案健檢" | 四維度專案分析 |
| `cta-trend-watch` | "趨勢"、"燃燒率" | 用量趨勢分析 |
| `cta-usage-pattern` | "使用模式"、"工作流建議" | 會話模式診斷與 harness 建議 |

歡迎台灣及亞洲開發者試用和回饋！
