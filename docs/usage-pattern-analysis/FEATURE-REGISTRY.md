# CTA Feature Registry — SSOT 功能清單

> **版本**：0.2.0-planned（含計劃中新功能）
> **建立日期**：2026-04-14
> **更新規則**：新增、修改、刪除任何 CTA 功能時必須同步更新此文件
> **SSOT**：`docs/usage-pattern-analysis/FEATURE-REGISTRY.md`

---

## 如何閱讀此文件

| 欄位 | 說明 |
|------|------|
| 功能 ID | 唯一識別碼，格式：`MCP-N`（MCP tool）/ `CLI-N`（CLI 命令）/ `SK-N`（Skill）|
| 狀態 | ✅ 已上線 / 🔄 開發中 / 📋 計劃中 |
| 測試覆蓋 | ✅ 有測試 / ⚠ 無直接測試（僅整合）/ ❌ 無測試 |

---

## A. MCP Tools（7 個現有 + 1 個計劃中）

### A1. 現有 MCP Tools

| 功能 ID | 名稱 | Rust 函式 | 輸入參數 | 輸出 | 狀態 | 測試覆蓋 | 文件位置 |
|---------|------|----------|---------|------|------|---------|---------|
| MCP-1 | `analyze_session` | `analyze_session_tool` | `session_id: String` | SessionAnalysis JSON | ✅ 已上線 | ⚠ 僅整合測試 | [tool-reference.md](../../skills/cta/references/tool-reference.md) |
| MCP-2 | `analyze_project` | `analyze_project_tool` | `project_path: String`, `sort_by?: String`, `limit?: u32`, `include_sessions?: bool` | ProjectAnalysis JSON | ✅ 已上線 | ⚠ 僅整合測試 | [tool-reference.md](../../skills/cta/references/tool-reference.md) |
| MCP-3 | `analyze_global` | `analyze_global_tool` | （無必填）| GlobalAnalysis JSON | ✅ 已上線 | ⚠ 僅整合測試 | [tool-reference.md](../../skills/cta/references/tool-reference.md) |
| MCP-4 | `cost_report` | `cost_report_tool` | `month?: String`, `daily?: bool`, `project?: String` | CostReport JSON | ✅ 已上線 | ⚠ 僅整合測試 | [tool-reference.md](../../skills/cta/references/tool-reference.md) |
| MCP-5 | `anomaly_scan` | `anomaly_scan_tool` | `threshold?: f64`, `project?: String`, `min_tokens?: u64` | AnomalyReport JSON | ✅ 已上線 | ⚠ 僅整合測試 | [tool-reference.md](../../skills/cta/references/tool-reference.md) |
| MCP-6 | `trend_report` | `trend_report_tool` | `granularity?: String`, `days?: u32`, `project?: String` | TrendReport JSON | ✅ 已上線 | ⚠ 僅整合測試 | [tool-reference.md](../../skills/cta/references/tool-reference.md) |
| MCP-7 | `sync_db` | `sync_db_tool` | （無必填）| SyncResult JSON | ✅ 已上線 | ⚠ 僅整合測試 | [tool-reference.md](../../skills/cta/references/tool-reference.md) |

### A2. 計劃中 MCP Tools

| 功能 ID | 名稱 | Rust 函式 | 輸入參數 | 輸出 | 狀態 | 測試覆蓋 | 文件位置 |
|---------|------|----------|---------|------|------|---------|---------|
| MCP-8 | `classify_session_pattern` | `classify_session_pattern_tool` | `session_id: String` | PatternResult JSON（見 SPEC.md §3.1）| 📋 計劃中（Phase 4）| 📋 計劃中（Phase 2）| [SPEC.md](SPEC.md) |

**測試覆蓋說明（MCP-1 ~ MCP-7）**：
- `bin/mcp.rs` 有 7 個 handler 函式，但**無直接單元測試**（0 tests in bin/）
- 覆蓋僅來自 `tests/integration.rs` 的 E2E 測試（15 個整合測試）
- Phase 2/3 重構時將 handler 邏輯移至 `mcp_lib.rs`，屆時補充單元測試

---

## B. CLI Commands（9 個入口點）

### B1. 現有 CLI Commands

| 功能 ID | 命令 | Rust 函式 | 描述 | 關鍵選項 | 狀態 | 測試覆蓋 | 對應 MCP Tool |
|---------|------|----------|------|---------|------|---------|--------------|
| CLI-1 | `cta sync` | `cmd_sync` | 同步 JSONL sessions 到 SQLite DB | `--verbose` | ✅ 已上線 | ❌ 無測試 | MCP-7 `sync_db` |
| CLI-2 | `cta analyze session <id>` | `cmd_analyze_session` | 分析單一 session | `<session_id>` | ✅ 已上線 | ❌ 無測試 | MCP-1 `analyze_session` |
| CLI-3 | `cta analyze project <path>` | `cmd_analyze_project` | 分析整個專案 | `<project_path>` | ✅ 已上線 | ❌ 無測試 | MCP-2 `analyze_project` |
| CLI-4 | `cta analyze global` | `cmd_analyze_global` | 分析全域使用狀況 | — | ✅ 已上線 | ❌ 無測試 | MCP-3 `analyze_global` |
| CLI-5 | `cta cost` | `cmd_cost` | 月度成本報表 | `--month`, `--daily`, `--project` | ✅ 已上線 | ❌ 無測試 | MCP-4 `cost_report` |
| CLI-6 | `cta archive` | `cmd_archive` | 歸檔舊 JSONL（zstd 壓縮）| `--dry-run`, `--days` | ✅ 已上線 | ❌ 無測試 | — |
| CLI-7 | `cta export` | `cmd_export` | 匯出 CSV 或 JSON | `--format`, `--output`, `--project` | ✅ 已上線 | ❌ 無測試 | — |
| CLI-8 | `cta anomalies` | `cmd_anomalies` | 偵測異常 sessions | `--threshold`, `--project` | ✅ 已上線 | ❌ 無測試 | MCP-5 `anomaly_scan` |
| CLI-9 | `cta trend` | `cmd_trend` | 使用趨勢報表 | `--granularity`, `--days`, `--project` | ✅ 已上線 | ❌ 無測試 | MCP-6 `trend_report` |

**測試覆蓋說明（CLI-1 ~ CLI-9）**：
- `bin/cli.rs`（917 LOC）**完全無單元測試**
- Phase 2 補齊 `tests/cli_unit.rs`
- Phase 3 重構後邏輯移至 `cli_lib.rs`，補充 lib 層單元測試

---

## C. Skills（6 個現有 + 1 個計劃中）

### C1. 現有 Skills

| 功能 ID | Skill ID | 觸發關鍵詞（摘要）| 主要功能 | 呼叫的 MCP Tools | 狀態 | 測試覆蓋 | 文件位置 |
|---------|----------|-----------------|---------|----------------|------|---------|---------|
| SK-1 | `cta` | "token analysis", "costs", "session analysis", "cta" | 路由器，分派到 SK-2 ~ SK-6 | 視路由而定 | ✅ 已上線 | ❌ 無自動測試 | [skills/cta/SKILL.md](../../skills/cta/SKILL.md) |
| SK-2 | `cta-health-check` | "quick check", "overview", "看看狀況", "總覽" | 一頁式快速總覽 | MCP-7, MCP-3, MCP-5 | ✅ 已上線 | ❌ 無自動測試 | [skills/cta-health-check/SKILL.md](../../skills/cta-health-check/SKILL.md) |
| SK-3 | `cta-cost-audit` | "monthly costs", "cost report", "這個月花多少", "預算" | 月度成本審計 | MCP-7, MCP-4 | ✅ 已上線 | ❌ 無自動測試 | [skills/cta-cost-audit/SKILL.md](../../skills/cta-cost-audit/SKILL.md) |
| SK-4 | `cta-anomaly-hunt` | "anomalies", "有異常嗎", "排查", "診斷" | 6 類異常掃描 | MCP-7, MCP-5, MCP-1 | ✅ 已上線 | ❌ 無自動測試 | [skills/cta-anomaly-hunt/SKILL.md](../../skills/cta-anomaly-hunt/SKILL.md) |
| SK-5 | `cta-project-review` | "analyze this project", "分析這個專案", "tool usage" | 專案四維分析 | MCP-7, MCP-2 | ✅ 已上線 | ❌ 無自動測試 | [skills/cta-project-review/SKILL.md](../../skills/cta-project-review/SKILL.md) |
| SK-6 | `cta-trend-watch` | "trends", "趨勢", "burn rate", "預測" | 趨勢與燃率分析 | MCP-7, MCP-6 | ✅ 已上線 | ❌ 無自動測試 | [skills/cta-trend-watch/SKILL.md](../../skills/cta-trend-watch/SKILL.md) |

### C2. 計劃中 Skills

| 功能 ID | Skill ID | 觸發關鍵詞（草案）| 主要功能 | 呼叫的 MCP Tools | 狀態 | 測試覆蓋 | 文件位置 |
|---------|----------|-----------------|---------|----------------|------|---------|---------|
| SK-7 | `cta-usage-pattern` | "使用模式", "pattern 分析", "harness 優化", "ASCII 圖", "工作流建議" | 使用模式分類 + Harness 建議 + ASCII 視覺化 | MCP-7, MCP-8 `classify_session_pattern` | 📋 計劃中（Phase 4）| 📋 計劃中（Phase 2）| [SPEC.md](SPEC.md) |

---

## D. 後端模組（Rust lib crate）

| 模組 | 主要職責 | LOC | 單元測試數 | 測試狀態 | 備註 |
|------|---------|-----|----------|---------|------|
| `config.rs` | 三模式路徑解析（env / plugin / standalone）| 181 | 6 | ✅ | Phase 3 加 errors.rs |
| `types.rs` | 核心資料結構定義 | 83 | 0 | ⚠ | 純 data struct，無邏輯 |
| `parser.rs` | JSONL 解析、dedup、compression 偵測 | 820 | 10 | ✅ | Phase 3 拆分為 parser/ 模組 |
| `pricing.rs` | TOML 定價、embedded fallback | 267 | 7 | ✅ | |
| `storage.rs` | SQLite schema + 24 查詢方法 | 1,668 | 13 | ✅ | Phase 3 拆分為 storage/ 模組 |
| `analyzer.rs` | Session/Project/Global/Trend 聚合 | 906 | 7 | ✅ | Phase 4 可能新增 signal 欄位 |
| `detector.rs` | 6 類異常 + 壓縮分析 | 1,017 | 12 | ✅ | |
| `archiver.rs` | zstd 壓縮歸檔 | 399 | 5 | ✅ | |
| `format.rs` | Token/Cost/CSV 格式化 | 209 | 15 | ✅ | |
| `session_finder.rs` | 遞迴搜尋 JSONL 檔案 | 101 | 5 | ✅ | |
| `bin/cli.rs` | 9 個 CLI 入口點（command handlers）| 917 | **0** ⚠ | ❌ | Phase 2 補測試，Phase 3 → cli_lib.rs |
| `bin/mcp.rs` | 7 個 MCP tool handlers | 472 | **0** ⚠ | ❌ | Phase 2 補測試，Phase 3 → mcp_lib.rs |

### D1. 計劃中新增模組

| 模組 | 主要職責 | 狀態 | Phase |
|------|---------|------|-------|
| `pattern_classifier.rs` | Session pattern 分類（硬訊號）| 📋 計劃中 | 4 |
| `cli_lib.rs` | CLI handler 邏輯（從 bin/cli.rs 移出）| 📋 計劃中 | 3 |
| `mcp_lib.rs` | MCP handler 邏輯（從 bin/mcp.rs 移出）| 📋 計劃中 | 3 |
| `errors.rs` | 自訂 Error enum（ConfigError 等）| 📋 計劃中 | 3 |
| `storage/mod.rs` | storage.rs 拆分後的入口 | 📋 計劃中 | 3 |
| `storage/schema.rs` | CREATE TABLE + migration | 📋 計劃中 | 3 |
| `storage/queries.rs` | 24 查詢方法 | 📋 計劃中 | 3 |
| `parser/mod.rs` | parser.rs 拆分後的入口 | 📋 計劃中 | 3 |
| `parser/jsonl.rs` | 單行 JSONL 解析 | 📋 計劃中 | 3 |
| `parser/dedup.rs` | partial/final dedup 邏輯 | 📋 計劃中 | 3 |
| `parser/compression_detector.rs` | 壓縮事件辨識 | 📋 計劃中 | 3 |

---

## E. 測試覆蓋彙整

### E1. 現有測試（Phase 0 盤點確認，共 98 個）

| 測試檔案 | 類型 | 測試數 | 涵蓋功能 |
|---------|------|--------|---------|
| `src/config.rs` | 單元 | 6 | 三模式路徑解析（env/plugin/standalone）|
| `src/parser.rs` | 單元 | 10 | JSONL 解析、dedup、compression |
| `src/pricing.rs` | 單元 | 7 | 定價查詢、fallback |
| `src/storage.rs` | 單元 | 13 | schema + 24 查詢方法 |
| `src/analyzer.rs` | 單元 | 7 | session/project/global 聚合 |
| `src/detector.rs` | 單元 | 12 | 6 類異常 + compression |
| `src/archiver.rs` | 單元 | 5 | zstd 壓縮歸檔 |
| `src/format.rs` | 單元 | 15 | token/cost/CSV 格式化 |
| `src/session_finder.rs` | 單元 | 5 | 遞迴搜尋 |
| `tests/integration.rs` | 整合 | 15 | E2E pipeline（含邊界條件）|
| `src/bin/cli.rs` | — | **0** ⚠ | 無 |
| `src/bin/mcp.rs` | — | **0** ⚠ | 無 |

**總計**：98（83 單元 + 15 整合）

### E2. 計劃中新增測試（Phase 2）

| 測試檔案 | 類型 | 預計測試數 | 涵蓋功能 |
|---------|------|----------|---------|
| `tests/cli_unit.rs` | 單元/整合 | ~18 | CLI-1 ~ CLI-9 各命令邏輯 |
| `tests/mcp_unit.rs` | 單元/整合 | ~14 | MCP-1 ~ MCP-7 handler 邏輯 |
| `tests/pattern_classifier.rs` | 單元 | ~20 | MCP-8 分類器（每 pattern ≥ 2 golden cases）|
| `tests/usage_pattern_e2e.rs` | E2E | ~5 | SK-7 黃金路徑 |
| `tests/signal_recommendation_mapping.rs` | 規則測試 | ~10 | 訊號 → 建議關鍵詞 assertion |
| `tests/fixtures/golden_sessions/` | Fixture | 20-50 JSONL | Pattern 分類 golden set |
| `src/config.rs`（補充）| 單元 | +5 | archive dir paths（PR #7 缺失）|

**預計新增後總計**：~170 個測試

---

## F. 環境變數（SSOT）

> 此為唯一環境變數清單。`README.md` 中的描述應與此表保持一致。`config.rs` 中的實作是執行時行為。

| 環境變數 | 用途 | 預設值 | 優先順序 |
|---------|------|--------|---------|
| `CLAUDE_PLUGIN_ROOT` | Plugin 模式根目錄（覆蓋所有路徑）| — | 最高 |
| `CLAUDE_CONFIG_DIR` | 替代 `$HOME/.claude/`（PR #7 新增）| — | 次高 |
| `CTA_DB_PATH` | SQLite DB 路徑覆蓋 | `$CLAUDE_PLUGIN_ROOT/data/token-analyzer.db` 或 `$HOME/.claude/data/token-analyzer.db` | — |
| `CTA_PROJECTS_DIR` | Projects 目錄覆蓋 | `$HOME/.claude/projects/`（不受 plugin mode 影響）| — |
| `CTA_ARCHIVE_DIR` | 歸檔目錄覆蓋 | `$CLAUDE_PLUGIN_ROOT/data/token-analyzer-archive` 或 `$HOME/.claude/data/token-analyzer-archive` | — |
| `CTA_PRICING_PATH` | 定價 TOML 覆蓋 | embedded `config/pricing.toml` | — |

---

## G. 開放 GitHub Issues（追蹤）

| Issue # | 標題 | 與本計劃關係 |
|---------|------|------------|
| #7（PR）| Support CLAUDE_CONFIG_DIR for path resolution | Phase 2.3 補 5 個測試 → Phase 6.1 協助 merge |
| #6 | Support CLAUDE_CONFIG_DIR for path resolution | 對應 PR #7 |
| #5 | Add daily/weekly cost summary notifications | 不在本次範圍 |
| #4 | Add session archival with zstd compression | 已實作（archiver.rs）|
| #3 | Add per-model cost and cache hit rate breakdown | 已部分實作（analyzer.rs）|
| #2 | Support custom anomaly thresholds via config | 不在本次範圍 |
| #1 | Add CSV export for cost reports | 已實作（CLI-7 export）|
| 新建 | Feature: Usage Pattern Analysis Skill | Phase 6.2 建立 |

---

## 更新歷史

| 日期 | 版本 | 變更 |
|------|------|------|
| 2026-04-14 | 0.2.0-planned | 初始建立，基於 Phase 0 架構盤點 |
