# CTA Usage Pattern Analysis — 完整工程計劃

> **狀態**：Phase 1 (SDD) — 規格制定執行中
> **建立日期**：2026-04-14
> **計劃檔案路徑**：`/Users/liyuefong/.claude/plans/fizzy-meandering-hickey.md`
> **最終輸出位置**：`docs/usage-pattern-analysis/PLAN.md`（此為 repo 內 SSOT，以此為準）
>
> **每次恢復上下文時必讀此檔案**。這是唯一的 SSOT（Single Source of Truth）。

---

## Context（背景）

### 為什麼需要這個改動

使用者使用 CTA 現有 skills（cta, cta-health-check, cta-cost-audit, cta-anomaly-hunt, cta-project-review, cta-trend-watch）進行 session 分析時，發現：

1. **既有 skill 系統覆蓋範圍狹窄**：只回答「成本多少」「有無異常」「單一專案狀況」這類結構化問題
2. **無法回答 meta-layer 問題**：使用者的整體使用模式（Marathon 模式、Observer 模式、週節奏）
3. **無 ASCII 視覺化**：純表格輸出難以一眼辨識趨勢、分佈、異常熱點
4. **無 Harness 優化建議**：無法回饋 Claude Code harness（hooks、skills、tools）的優化方向
5. **無使用流程建議**：無法給出 best-practice workflow（何時 checkpoint、何時開 plan mode、何時用 subagent）

### 現況觸發

在 session `2026-04-14 13:50` 中，使用者要求 Claude 透過 CTA MCP tools 做一次「整體使用模式分析 + ASCII 圖 + Harness 優化建議」。Claude 成功合成了報告，但這個能力未內建在 skill 系統中——每次都要靠主 agent 臨場合成，不可複用、不可驗證、不可測試。

### 預期成果

1. **新 skill `cta-usage-pattern`**：正式化此分析流程，可由路由器 `cta` 自動路由
2. **Harness Engineer 最佳實踐文件**：基於研究的系統化建議庫
3. **完整測試套件**：單元測試 + 整合測試 + 黃金路徑 E2E
4. **開源貢獻**：
   - 建立 GitHub Issue 描述此功能
   - 一併解決既有 PR「Support CLAUDE_CONFIG_DIR for path resolution」
5. **SSOT 文件**：將所有設計、進度、決策集中在 `docs/usage-pattern-analysis/` 下

---

## Guiding Principles（指導原則）

> 以下原則來自使用者明示，是**不可協商**的工程守則：

1. **正確 > 快速** — 快速沒用，正確才有意義，快思慢想
2. **規格先行 SDD → 測試先行 TDD/BDD → 實作** — 嚴格順序
3. **先重構再加功能** — 重構時考慮新功能，避免雙重改動
4. **100% 測試覆蓋** — 斷言有效且完整，禁止偽斷言、隱藏紅燈
5. **SSOT** — 文件不散落，集中在 `docs/usage-pattern-analysis/`
6. **驗證沒驗證出來的 bug = 測試不足** — 發現就補齊測試，不因工作量大而跳過
7. **禁止 hardcode** — 任何魔術數字／路徑要集中管理
8. **禁止 bad smell**：過度設計、未關注點分離、錯誤 Design Pattern
9. **交叉驗證 Review**：linus-review-framework + Codex CLI 雙審
10. **逐步交付** — 一次一個可驗證單元，完成即提交

---

## Phase 結構（本計劃涵蓋至交付）

```
Phase 0: 情報收集（✅ 完成）
├── Agent-1: 現行架構 & 測試覆蓋盤點
├── Agent-2: GitHub 狀態 & CLAUDE_CONFIG_DIR PR 分析
└── Agent-3: Harness Engineer 最佳實踐研究

Phase 1: 規格制定（SDD）← 執行中
├── Spec 文件：功能需求、輸入輸出、邊界條件
├── BDD Feature 檔案：驗收場景
└── Architecture Decision Records（ADR）

Phase 2: 測試先行（TDD/BDD）
├── 單元測試：每個新函式/模組的失敗測試
├── 整合測試：MCP tool + skill 層級
├── 黃金路徑 E2E：真實 JSONL → skill 執行 → 報告輸出
└── 既有測試補漏：架構盤點發現的測試缺口

Phase 3: 重構（新功能前）
├── 處理架構盤點發現的 bad smell
├── 抽出將被 reused 的模組
└── 測試維持綠燈

Phase 4: 新功能實作
├── 若需新增 MCP tool（如 session_pattern_analyze）→ backend 變更
├── 新 skill 檔案：skills/cta-usage-pattern/SKILL.md
├── 更新 cta router 路由表
└── Harness 建議資料庫：skills/cta-usage-pattern/references/harness-best-practices.md

Phase 5: 交叉驗證 Review
├── linus-review-framework 全量 review
├── Codex CLI 二審
└── 修正 review 發現

Phase 6: 開源交付
├── PR：Support CLAUDE_CONFIG_DIR for path resolution（先處理，解除技術債）
├── Issue：Feature: Usage Pattern Analysis Skill
└── PR：新功能實作

Phase 7: 文件歸檔
├── docs/usage-pattern-analysis/PLAN.md（本計劃 dump）← 此檔案
├── docs/usage-pattern-analysis/HARNESS-BEST-PRACTICES.md
├── docs/usage-pattern-analysis/ARCHITECTURE-AUDIT.md
└── 更新 CLAUDE.md 指向此文件
```

---

## Phase 0: 情報收集結果（2026-04-14 完成並驗證）

### 0.1 現行架構（Agent-1 盤點，已驗證）

**模組地圖**（總計 5,662 LOC / 83 單元測試 / 15 整合測試 / 零 hardcode）

| 檔案 | 職責 | LOC | 單元測試數 |
|------|------|-----|------------|
| `config.rs` | 三模式路徑解析（env override / plugin / standalone） | 181 | 6 |
| `types.rs` | 核心資料結構 | 83 | 0 |
| `parser.rs` | JSONL 解析、dedup、compression 偵測 | 820 | 10 |
| `pricing.rs` | TOML 定價、embedded fallback、`$CTA_PRICING_PATH` | 267 | 7 |
| `storage.rs` | SQLite schema + 24 查詢方法 | 1,668 | 13 |
| `analyzer.rs` | Session/Project/Global/Trend/Cost 聚合 | 906 | 7 |
| `detector.rs` | 6 類異常 + 壓縮分析 | 1,017 | 12 |
| `archiver.rs` | zstd 壓縮歸檔 | 399 | 5 |
| `format.rs` | Token/Cost/CSV 格式化 | 209 | 15 |
| `session_finder.rs` | 遞迴搜尋 JSONL | 101 | 5 |
| `bin/cli.rs` | 9 個 CLI 子命令 | 917 | **0** ⚠ |
| `bin/mcp.rs` | 7 個 MCP tool handlers（rmcp + `#[tool]`） | 472 | **0** ⚠ |

**測試現況**：
- 單元測試 83（src 9 個模組） + 整合測試 15（tests/integration.rs）= **98 總計**
- 覆蓋薄弱：`bin/cli.rs`（917 LOC 無單元測試）、`bin/mcp.rs`（472 LOC 無單元測試）
- 非測試程式碼 0 個 `panic!()`、0 個 production `unwrap()`

**Bad smells 與技術債**：
1. ⚠ **bin/ 無單元測試**：CLI 子命令與 MCP tool handler 邏輯未隔離驗證，僅依賴整合測試
2. ⚠ **storage.rs 接近 God Module**（1,668 LOC）：schema + 24 查詢混在一起
3. ⚠ **parser.rs 3 職責混合**：parse + dedup + compression detection
4. ⚠ **anyhow 錯誤無根因區分**：無 HOME vs 無寫權限看不出來
5. ⚠ **test 中 `unsafe` env 操作**：用 Mutex 隔離但可用 DI 改善
6. ✓ **零 hardcode**：路徑均由 env var + 函式解析，定價在 TOML

---

### 0.2 GitHub 狀態（Agent-2 盤點，已驗證）

**PR #7** — Support CLAUDE_CONFIG_DIR for path resolution
- 作者：halindrome（外部貢獻者）
- 狀態：OPEN / CHANGES_REQUESTED / MERGEABLE
- Diff：+127/-22，4 檔案（config.rs +119/-14, mcp.rs +1/-1, CLAUDE.md +3/-3, README.md +4/-4）
- CI：本地 103/103 tests 通過

**Blocking review comments（5 項缺失測試）**：
1. `test_resolve_archive_dir_from_env`
2. `test_resolve_archive_dir_plugin_mode`
3. `test_resolve_archive_dir_standalone`
4. `test_plugin_root_takes_priority_over_config_dir_for_archive`
5. `test_coexistence_plugin_root_and_config_dir_split_behavior`

**技術債**：無 `cargo test` CI workflow（PR 無自動驗證）⚠

---

### 0.3 Harness Engineer 最佳實踐研究（Agent-3 合成）

**Session 類型分類**：

| 類型 | 訊號 | 閾值（初始） |
|------|------|------------|
| Marathon | turns >100 + duration >2h + cache hit >70% | 正常模式 |
| Observer | turns <20 + Read/Grep 主力 + edit 少 | 偵察模式 |
| Kitchen Sink ⚠ | 話題跳動 >3 + 重複 Read 同檔 | Anti-pattern |
| Correction Spiral ⚠ | 同檔 edit >3 + output token 遞增 | Anti-pattern |
| Subagent Swarm ⚠ | 單 session >10 個 Task | Anti-pattern |
| Cold Session ⚠ | cache_read / input < 0.3 | cache 保溫失敗 |

**架構建議**：新增薄 MCP tool `classify_session_pattern` 回傳硬訊號；skill 層 LLM 合成建議文字。

---

## 使用者決策（2026-04-14 確認）

| 決策 | 選擇 |
|------|------|
| PR #7 處理 | 協助 halindrome 補 5 項測試，備 patch / comment，由作者 force push |
| 架構方向 | 混合：薄 Rust MCP tool `classify_session_pattern` + skill 層 LLM 合成建議 |
| 重構範疇 | 完整：架構修復（storage/parser/bin 拆分）+ 測試補齊 + CI test workflow |

---

## 目標檔案地圖

### 新增
```
docs/usage-pattern-analysis/
├── PLAN.md                            # 本計劃（此檔案）
├── SPEC.md                            # 功能規格 ← Phase 1
├── FEATURE-REGISTRY.md                # SSOT 功能清單 ← Phase 1
├── HARNESS-BEST-PRACTICES.md          # Agent-3 研究歸檔 ← Phase 7
├── ARCHITECTURE-AUDIT.md              # Agent-1 盤點歸檔 ← Phase 7
├── ADR/
│   ├── 0001-hybrid-mcp-skill.md       # ← Phase 1
│   ├── 0002-pattern-classification-thresholds.md  # ← Phase 1
│   └── 0003-ascii-viz-strategy.md     # ← Phase 1
└── features/
    ├── classify_session_pattern.feature  # ← Phase 1
    ├── generate_recommendations.feature  # ← Phase 1
    └── ascii_sparkline.feature           # ← Phase 1

mcp-server/src/
├── pattern_classifier.rs              # 新：硬訊號分類器 ← Phase 4
├── storage/                           # 重構：拆分 storage.rs ← Phase 3
│   ├── mod.rs
│   ├── schema.rs
│   └── queries.rs
├── parser/                            # 重構：拆分 parser.rs ← Phase 3
│   ├── mod.rs
│   ├── jsonl.rs
│   ├── dedup.rs
│   └── compression_detector.rs
├── cli_lib.rs                         # 重構：bin/cli.rs 邏輯移至 lib ← Phase 3
├── mcp_lib.rs                         # 重構：bin/mcp.rs 邏輯移至 lib ← Phase 3
└── errors.rs                          # 新：自訂 Error enum ← Phase 3

mcp-server/tests/
├── pattern_classifier.rs              # ← Phase 2
├── usage_pattern_e2e.rs               # ← Phase 2
├── cli_unit.rs                        # ← Phase 2
└── fixtures/golden_sessions/          # ← Phase 2

skills/cta-usage-pattern/
├── SKILL.md                           # ← Phase 4
└── references/
    ├── harness-signals-to-advice.md   # ← Phase 4
    └── pattern-definitions.md         # ← Phase 4

.github/workflows/test.yml             # ← Phase 3（CI 補齊）
```

### 修改
```
mcp-server/src/bin/mcp.rs              # 新增 classify_session_pattern ← Phase 4
mcp-server/src/lib.rs                  # 更新 mod 宣告 ← Phase 3
.mcp.json                              # 補 CTA_PROJECTS_DIR, CTA_ARCHIVE_DIR ← Phase 3
CLAUDE.md                              # 指向 docs/usage-pattern-analysis/ ← Phase 7
skills/cta/SKILL.md                    # 更新路由表 ← Phase 4
README.md                              # 新增 Usage Pattern Analysis 段落 ← Phase 6
```

---

## 當前進度

| Phase | 狀態 | 完成日期 | 備註 |
|-------|------|---------|------|
| 0 — 情報收集 | ✅ 完成 | 2026-04-14 | 3 Explore agents + 13 項驗證 |
| 1 — 規格制定 | 🔄 執行中 | — | SPEC.md / FEATURE-REGISTRY / BDD / ADRs |
| 2 — 測試先行 | 未開始 | — | 等 Phase 1 |
| 3 — 重構 | 未開始 | — | 等 Phase 2 |
| 4 — 新功能 | 未開始 | — | 等 Phase 3 |
| 5 — 交叉 Review | 未開始 | — | linus-review-framework + Codex CLI |
| 6 — 開源交付 | 未開始 | — | PR #7 先行，再 new PR |
| 7 — 文件歸檔 | 未開始 | — | 與 Phase 6 並行 |

---

## 脈絡恢復協議（Context Recovery Protocol）

**每次新 session 開始或 compact 後，必須：**

1. 讀取此檔案完整內容（`docs/usage-pattern-analysis/PLAN.md`）
2. 查看「當前進度」區段定位所處 Phase
3. 若本檔案與 `~/.claude/plans/fizzy-meandering-hickey.md` 不一致，以此 repo 版本為準
4. Phase 2+ 開始前確認：`cargo test --all-targets` 仍全綠（執行驗證）

---

## 關鍵風險與緩解

| 風險 | 影響 | 緩解 |
|------|------|------|
| PR #7 作者長期不回應 | 阻塞 Phase 6 | 給 halindrome 7 天，逾期詢問 repo owner |
| storage.rs 拆分破壞整合測試 | 回歸 | Phase 2.1 補 query-level 單元測試，Phase 3 逐方法搬移 |
| pattern 閾值定義爭議 | 建議品質差 | ADR-0002 明列：初始值來自研究，Phase 4 後用實測樣本校準 |
| chartli crate 維護風險 | 長期相依負擔 | 自寫 sparkline 為 baseline，chartli 為 optional feature |
| CLAUDE_CONFIG_DIR 與 config error 重構衝突 | Phase 6 rebase 痛 | 先等 PR #7 merge 再做 config.rs error 重構 |
