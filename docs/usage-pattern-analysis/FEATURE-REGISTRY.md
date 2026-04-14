# CTA Feature Registry — SSOT 功能清單

> **版本**：0.3.0-implemented
> **建立日期**：2026-04-14
> **最後更新**：2026-04-14
> **SSOT**：`docs/usage-pattern-analysis/FEATURE-REGISTRY.md`
> **範圍規則**：此文件只列出 repo 內已存在且可交付的功能、模組、測試與環境變數。未來重構與 backlog 一律記錄在 `PLAN.md`，不在此處混寫。

---

## 如何閱讀此文件

| 欄位 | 說明 |
|------|------|
| 功能 ID | 唯一識別碼，格式：`MCP-N` / `CLI-N` / `SK-N` |
| 狀態 | `✅ 已上線` / `⚠ 部分覆蓋` |
| 測試覆蓋 | `✅ 直接測試` / `⚠ 間接或部分測試` / `❌ 無直接測試` |

---

## A. MCP Tools（8 個現有）

| 功能 ID | 名稱 | Rust 函式 | 輸入參數 | 輸出 | 狀態 | 測試覆蓋 | 文件位置 |
|---------|------|----------|---------|------|------|---------|---------|
| MCP-1 | `analyze_session` | `analyze_session_tool` | `session_id: String` | `SessionAnalysis` JSON | ✅ 已上線 | ⚠ 整合測試 | [tool-reference.md](../../skills/cta/references/tool-reference.md) |
| MCP-2 | `analyze_project` | `analyze_project_tool` | `project_path: String`, `include_subagents?: bool`, `sort_by?: String`, `limit?: u32` | `ProjectAnalysis` JSON | ✅ 已上線 | ⚠ 整合測試 | [tool-reference.md](../../skills/cta/references/tool-reference.md) |
| MCP-3 | `analyze_global` | `analyze_global_tool` | 無 | `GlobalAnalysis` JSON | ✅ 已上線 | ⚠ 整合測試 | [tool-reference.md](../../skills/cta/references/tool-reference.md) |
| MCP-4 | `cost_report` | `cost_report_tool` | `month?: String`, `daily?: bool`, `project_path?: String` | `CostReport` JSON | ✅ 已上線 | ⚠ 整合測試 | [tool-reference.md](../../skills/cta/references/tool-reference.md) |
| MCP-5 | `anomaly_scan` | `anomaly_scan_tool` | `stddev_threshold?: f64`, `project_path?: String`, `min_tokens_for_cache_check?: u64` | `AnomalyReport` JSON | ✅ 已上線 | ⚠ 整合測試 | [tool-reference.md](../../skills/cta/references/tool-reference.md) |
| MCP-6 | `trend_report` | `trend_report_tool` | `granularity?: String`, `last_n_days?: u32`, `project_path?: String` | `TrendAnalysis` JSON | ✅ 已上線 | ⚠ 整合測試 | [tool-reference.md](../../skills/cta/references/tool-reference.md) |
| MCP-7 | `sync_db` | `sync_db_tool` | 無 | `SyncReport` JSON | ✅ 已上線 | ⚠ 整合測試 | [tool-reference.md](../../skills/cta/references/tool-reference.md) |
| MCP-8 | `classify_session_pattern` | `classify_session_pattern_tool` | `session_id: String`（完整 UUID 或唯一前綴） | `PatternResult` JSON | ✅ 已上線 | ✅ 直接單元測試 + classifier/signal coverage | [SPEC.md](SPEC.md) |

**補充說明**
- `classify_session_pattern` 直接讀取 JSONL，不依賴 SQLite 是否已同步。
- `classify_session_pattern` 錯誤碼為：`SESSION_NOT_FOUND`、`AMBIGUOUS_SESSION_ID`、`PARSE_FAILED`、`INSUFFICIENT_DATA`。
- 其餘 7 個工具目前以 `tests/integration.rs` 為主；`bin/mcp.rs` 只有 MCP-8 具備直接 handler-level 單元測試。

---

## B. CLI Commands（9 個現有）

| 功能 ID | 命令 | Rust 函式 | 描述 | 關鍵選項 | 狀態 | 測試覆蓋 | 對應 MCP Tool |
|---------|------|----------|------|---------|------|---------|--------------|
| CLI-1 | `cta sync` | `cmd_sync` | 同步 JSONL sessions 到 SQLite DB | `--verbose` | ✅ 已上線 | ⚠ `tests/cli_unit.rs` smoke/validation | MCP-7 |
| CLI-2 | `cta analyze session <id>` | `cmd_analyze_session` | 分析單一 session | `<session_id>` | ✅ 已上線 | ⚠ `tests/cli_unit.rs` smoke/validation | MCP-1 |
| CLI-3 | `cta analyze project <path>` | `cmd_analyze_project` | 分析整個專案 | `<project_path>` | ✅ 已上線 | ⚠ `tests/cli_unit.rs` smoke/validation | MCP-2 |
| CLI-4 | `cta analyze global` | `cmd_analyze_global` | 分析全域使用狀況 | 無 | ✅ 已上線 | ⚠ `tests/cli_unit.rs` smoke/validation | MCP-3 |
| CLI-5 | `cta cost` | `cmd_cost` | 月度成本報表 | `--month`, `--daily`, `--project` | ✅ 已上線 | ⚠ `tests/cli_unit.rs` smoke/validation | MCP-4 |
| CLI-6 | `cta archive` | `cmd_archive` | 歸檔舊 JSONL（zstd 壓縮） | `--dry-run`, `--days` | ✅ 已上線 | ⚠ `tests/cli_unit.rs` smoke/validation | — |
| CLI-7 | `cta export` | `cmd_export` | 匯出 CSV 或 JSON | `--format`, `--output`, `--project` | ✅ 已上線 | ⚠ `tests/cli_unit.rs` smoke/validation | — |
| CLI-8 | `cta anomalies` | `cmd_anomalies` | 偵測異常 sessions | `--threshold`, `--project` | ✅ 已上線 | ⚠ `tests/cli_unit.rs` smoke/validation | MCP-5 |
| CLI-9 | `cta trend` | `cmd_trend` | 使用趨勢報表 | `--granularity`, `--days`, `--project` | ✅ 已上線 | ⚠ `tests/cli_unit.rs` smoke/validation | MCP-6 |

**補充說明**
- CLI 目前有 10 個 smoke/validation 測試，但 `bin/cli.rs` 本身仍沒有直接單元測試。
- `cli_lib.rs` 已存在並承接部分可測試邏輯，但尚未成為唯一執行入口。

---

## C. Skills（7 個現有）

| 功能 ID | Skill ID | 觸發關鍵詞（摘要） | 主要功能 | 呼叫的 MCP Tools | 狀態 | 測試覆蓋 | 文件位置 |
|---------|----------|-------------------|---------|----------------|------|---------|---------|
| SK-1 | `cta` | `token analysis`, `costs`, `session analysis`, `cta` | CTA 路由器，分派到子 skill | 視路由而定 | ✅ 已上線 | ❌ 無自動測試 | [skills/cta/SKILL.md](../../skills/cta/SKILL.md) |
| SK-2 | `cta-health-check` | `quick check`, `overview`, `總覽` | 一頁式快速總覽 | MCP-7, MCP-3, MCP-5 | ✅ 已上線 | ❌ 無自動測試 | [skills/cta-health-check/SKILL.md](../../skills/cta-health-check/SKILL.md) |
| SK-3 | `cta-cost-audit` | `monthly costs`, `cost report`, `預算` | 月度成本審計 | MCP-7, MCP-4 | ✅ 已上線 | ❌ 無自動測試 | [skills/cta-cost-audit/SKILL.md](../../skills/cta-cost-audit/SKILL.md) |
| SK-4 | `cta-anomaly-hunt` | `anomalies`, `排查`, `診斷` | 6 類異常掃描 | MCP-7, MCP-5, MCP-1 | ✅ 已上線 | ❌ 無自動測試 | [skills/cta-anomaly-hunt/SKILL.md](../../skills/cta-anomaly-hunt/SKILL.md) |
| SK-5 | `cta-project-review` | `analyze this project`, `tool usage` | 專案四維分析 | MCP-7, MCP-2 | ✅ 已上線 | ❌ 無自動測試 | [skills/cta-project-review/SKILL.md](../../skills/cta-project-review/SKILL.md) |
| SK-6 | `cta-trend-watch` | `trends`, `burn rate`, `預測` | 趨勢與燃率分析 | MCP-7, MCP-6 | ✅ 已上線 | ❌ 無自動測試 | [skills/cta-trend-watch/SKILL.md](../../skills/cta-trend-watch/SKILL.md) |
| SK-7 | `cta-usage-pattern` | `使用模式`, `pattern 分析`, `harness 優化`, `ASCII 圖` | 使用模式分類、訊號摘要、workflow 建議、可選 sparkline | MCP-8, MCP-6，視 freshness 需要使用 MCP-7 | ✅ 已上線 | ⚠ 文件/映射測試，尚無 skill runner 自動測試 | [skills/cta-usage-pattern/SKILL.md](../../skills/cta-usage-pattern/SKILL.md) |

---

## D. 後端模組（Rust crate 快照）

| 模組 | 主要職責 | 直接測試數 | 測試狀態 | 備註 |
|------|---------|-----------|---------|------|
| `config.rs` | DB / projects / archive 路徑解析與 precedence | 13 | ✅ | projects 支援 `CTA_PROJECTS_DIR` > `CLAUDE_CONFIG_DIR/projects` > `~/.claude/projects` |
| `types.rs` | 核心資料結構 | 0 | ⚠ | `ToolUseInfo.file_path` 已存在，供 pattern signals 使用 |
| `parser.rs` | JSONL 解析、dedup、compression 偵測、tool input 擷取 | 14 | ✅ | 已解析 `tool_use.input.file_path` |
| `pricing.rs` | TOML 定價與 embedded fallback | 7 | ✅ | — |
| `storage.rs` | SQLite schema 與查詢 | 13 | ✅ | — |
| `analyzer.rs` | Session/Project/Global/Trend 聚合 | 7 | ✅ | `total_turns` 為 assistant turn 數 |
| `detector.rs` | 異常偵測與 severity | 12 | ✅ | — |
| `archiver.rs` | zstd 歸檔 | 5 | ✅ | — |
| `format.rs` | token/cost/CSV 格式化 | 15 | ✅ | — |
| `session_finder.rs` | session JSONL 搜尋與前綴解析 | 9 | ✅ | 支援 exact match 與 unique prefix |
| `pattern_classifier.rs` | Session pattern 分類與 evidence contract | 28 | ✅ | `PatternResult` / `Evidence` 已序列化並接上 MCP |
| `pattern_signals.rs` | 從 `ParseResult` 萃取 signals | 2 | ✅ | `Agent`、`repeated_edit_peak`、duration、topic shift heuristic |
| `sparkline.rs` | Unicode sparkline renderer | 8 | ✅ | skill 層可選用於 trend summary |
| `cli_lib.rs` | 可測試的 CLI helper 邏輯 | 0 | ⚠ | 已存在，但尚未成為唯一 CLI 入口 |
| `bin/cli.rs` | CLI command handlers | 0 | ⚠ | 主要靠 `tests/cli_unit.rs` smoke/validation 覆蓋 |
| `bin/mcp.rs` | MCP tool handlers | 12 | ✅ | 含 `classify_session_pattern` handler happy/error path與 MCP error envelope |

---

## E. 測試覆蓋彙整（目前 repo 快照，共 172 個）

| 測試檔案 | 類型 | 測試數 | 涵蓋功能 |
|---------|------|--------|---------|
| `src/config.rs` | 單元 | 13 | 路徑 precedence 與 coexistence |
| `src/parser.rs` | 單元 | 14 | JSONL 解析、dedup、compression、`file_path` 擷取 |
| `src/pricing.rs` | 單元 | 7 | 定價查詢與 fallback |
| `src/storage.rs` | 單元 | 13 | schema 與查詢 |
| `src/analyzer.rs` | 單元 | 7 | session/project/global 聚合 |
| `src/detector.rs` | 單元 | 12 | 6 類異常與 compression |
| `src/archiver.rs` | 單元 | 5 | zstd 歸檔 |
| `src/format.rs` | 單元 | 15 | token/cost/CSV 格式化 |
| `src/session_finder.rs` | 單元 | 9 | recursive search、exact/prefix resolution |
| `src/sparkline.rs` | 單元 | 8 | Unicode sparkline rendering |
| `src/bin/mcp.rs` | 單元 | 12 | MCP-8 happy path、prefix/error mapping、error envelope |
| `tests/cli_unit.rs` | 整合/命令 | 10 | CLI validation 與 smoke coverage |
| `tests/pattern_classifier.rs` | 單元 | 28 | priority、severity、evidence、serde contract |
| `tests/pattern_signals.rs` | 單元 | 2 | `Agent` count、edit peak、duration、topic shift |
| `tests/signal_recommendation_mapping.rs` | 規則測試 | 2 | pattern → advice mapping completeness |
| `tests/integration.rs` | 整合 | 15 | 既有 E2E pipeline |

**總計**：172

**目前已知測試缺口**
- `bin/cli.rs` 尚無 direct handler-level 單元測試。
- skills 尚無自動執行測試。
- `types.rs`、`cli_lib.rs` 為低邏輯模組，暫無 direct tests。

---

## F. 環境變數（唯一清單）

> `README.md` 與 `CLAUDE.md` 對環境變數的描述必須與此表一致；執行時行為以 `config.rs` 為準。

| 環境變數 | 用途 | 預設值 | 優先順序 |
|---------|------|--------|---------|
| `CLAUDE_PLUGIN_ROOT` | Plugin 模式根目錄（只影響 DB / Archive） | — | DB / Archive 次高 |
| `CLAUDE_CONFIG_DIR` | Claude config root，提供 projects 預設位置 | — | Projects 次高 |
| `CTA_DB_PATH` | SQLite DB 路徑覆蓋 | `$CLAUDE_PLUGIN_ROOT/data/token-analyzer.db` 或 `$HOME/.claude/token-analyzer.db` | DB 最高 |
| `CTA_PROJECTS_DIR` | Projects 目錄覆蓋 | `$CLAUDE_CONFIG_DIR/projects` 或 `$HOME/.claude/projects` | Projects 最高 |
| `CTA_ARCHIVE_DIR` | 歸檔目錄覆蓋 | `$CLAUDE_PLUGIN_ROOT/data/token-analyzer-archive` 或 `$HOME/.claude/token-analyzer-archive` | Archive 最高 |
| `CTA_PRICING_PATH` | 定價 TOML 覆蓋 | embedded `config/pricing.toml` | — |

---

## G. 備註與邊界

- backlog、未來拆模組、長期重構不在此文件維護，避免 inventory 與 roadmap 混寫。
- `classify_session_pattern` 的 pattern、threshold 與 evidence contract 以 [SPEC.md](SPEC.md) 為準。
- `cta-usage-pattern` 的 workflow advice mapping 以 [harness-signals-to-advice.md](../../skills/cta-usage-pattern/references/harness-signals-to-advice.md) 為準。

---

## 更新歷史

| 日期 | 版本 | 變更 |
|------|------|------|
| 2026-04-14 | 0.2.0-planned | 初始建立，混合規劃與現況盤點 |
| 2026-04-14 | 0.3.0-implemented | 改寫為純 inventory SSOT，納入 MCP-8、SK-7、pattern_signals、sparkline、最新測試快照 |
