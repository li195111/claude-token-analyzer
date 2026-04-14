# CTA Usage Pattern Analysis — 功能規格文件（SPEC）

> **版本**：0.2.0-implemented
> **建立日期**：2026-04-14
> **狀態**：Implemented and verified in current working tree
> **SSOT 指引**：本文件為 `docs/usage-pattern-analysis/` 下的功能規格。
> 所有 pattern 定義、threshold 值、output 格式以此文件為準。

---

## 1. 功能目的（Purpose）

### 1.1 問題陳述

CTA 現有 skills（cta-health-check、cta-cost-audit 等）回答的是**結構化問題**（成本多少、有無異常）。
但 Harness Engineer 需要回答的是 **meta-layer 問題**：

- 「我的使用模式是 Marathon 還是 Observer？」
- 「哪些 session 是 Anti-pattern，應該如何調整工作流？」
- 「我的 harness（hooks、skills、tools）有哪些可以優化？」

### 1.2 解決方案

新增 `classify_session_pattern` MCP tool：
- 輸入：一個 session_id
- 輸出：pattern 標籤 + 硬訊號數值 + severity + evidence list
- 不包含建議文字（由 skill 層 LLM 合成）

新增 `cta-usage-pattern` skill：
- 路由「使用模式」「harness 優化」等關鍵詞
- 呼叫 MCP tool 取得分類結果
- 合成人類可讀的建議報告（含 ASCII 視覺化）

### 1.3 Non-Goals（本次不做）

| 項目 | 原因 |
|------|------|
| 跨 session 聚合分析 | Phase 1 scope，下個版本加 bulk 接口 |
| 自動閾值校準 | 閾值初始值由研究定，校準需要標記資料集（ADR-0002）|
| Real-time 監控 | CTA 是事後分析工具 |
| 建議文字由 Rust 生成 | 建議文字屬 LLM 合成，不放進 Rust binary |
| 跨 project 比較模式 | 需要額外聚合邏輯，延後至 v0.2 |

---

## 2. 資料輸入（Input Contract）

### 2.1 classify_session_pattern MCP Tool

```
Input:
  session_id: String   # 完整 session UUID 或唯一前綴，識別一個 JSONL 檔

Output:
  PatternResult (JSON，見 Section 3)
```

**session_id 解析規則**：
- 接受完整 UUID（如 `"abc12345-6789-..."`）
- 接受唯一前綴（如 `"abc12345"`）
- 使用 session_finder 的 exact-or-unique-prefix 邏輯
- 找不到 → 返回 MCP Error（`SESSION_NOT_FOUND`）
- 多個 session 同時符合前綴 → 返回 MCP Error（`AMBIGUOUS_SESSION_ID`）

### 2.2 cta-usage-pattern Skill 輸入

Skill 接受自然語言輸入，LLM 從中萃取：
- session_id（若使用者指定特定 session）
- 若無指定且使用者要看最新資料 → skill 層可先呼叫 `sync_db`，再取最近 N 個 session 分析
- date_range：可選，格式 `YYYY-MM-DD..YYYY-MM-DD`

---

## 3. 資料輸出（Output Contract）

### 3.1 PatternResult JSON Schema

```jsonc
{
  // 主要分類結果
  "pattern": "marathon" | "observer" | "kitchen_sink" |
             "correction_spiral" | "subagent_swarm" | "cold_session" | "normal",

  // 硬訊號數值（分類決策依據）
  "signals": {
    "cache_hit_rate": f64,          // cache_read_tokens / (input_tokens + cache_read_tokens + cache_creation_tokens)，範圍 [0.0, 1.0]
    "output_token_ratio": f64,      // output_tokens / (input_tokens + output_tokens)，範圍 [0.0, 1.0]
    "subagent_count": u32,          // session 中 Agent tool 呼叫次數
    "repeated_edit_peak": u32,      // 單一檔案在此 session 中被 Edit/Write/MultiEdit 的最高次數
    "turn_count": u32,              // assistant turn 數（與現有 parser/analyzer 一致）
    "duration_minutes": u32 | null, // session 持續時間（分鐘）；缺 timestamp 時為 null
    "topic_shift_count": u32        // 估算話題切換次數（heuristic，見 Section 4.7）
  },

  // 嚴重程度
  "severity": "info" | "warn" | "alert",

  // 觸發分類的具體證據
  "evidence": [
    {
      "metric": String,             // 訊號名稱（如 "cache_hit_rate"）
      "value": f64,                 // 實際測量值
      "threshold": f64,             // 觸發閾值
      "direction": "below" | "equal" | "above"  // 實際值相對閾值的方向
    }
  ]
}
```

### 3.2 MCP Error Contract

`rmcp` transport error code仍使用標準 numeric code；symbolic business error code 透過 `error.data.code` 傳遞。

| Symbolic Code (`error.data.code`) | transport `error.code` | 意義 |
|-----------------------------------|------------------------|------|
| `SESSION_NOT_FOUND` | `INVALID_PARAMS` | session_id 無法在 projects_dir 找到對應 JSONL |
| `AMBIGUOUS_SESSION_ID` | `INVALID_PARAMS` | session_id 前綴對應到多個 JSONL 檔 |
| `PARSE_FAILED` | `INVALID_REQUEST` | JSONL 格式錯誤，無法計算訊號 |
| `INSUFFICIENT_DATA` | `INVALID_PARAMS` | session 太短（< 3 assistant turns），無法有意義分類 |

---

## 4. Pattern 定義（Pattern Definitions）

### 4.1 決策優先順序（Priority Order）

當一個 session 符合多個 pattern 條件時，依以下優先順序決定最終分類：

```
Cold Session > Correction Spiral > Subagent Swarm > Kitchen Sink > Marathon > Observer > Normal
```

**設計理由**：Anti-pattern 優先於正常模式，最嚴重的訊號（cache 失效）最優先呈現。

---

### 4.2 Cold Session（冷 session）

**定義**：cache 保溫完全失敗，每次 turn 的 context 都重新讀取。

| 訊號 | 閾值 | 方向 |
|------|------|------|
| `cache_hit_rate` | < 0.30 | below |

**Severity**：
- `cache_hit_rate` < 0.15 → `alert`
- `cache_hit_rate` ∈ [0.15, 0.30) → `warn`

**常見原因**：
- session 每次重啟（無長對話）
- CLAUDE.md 頻繁更改（cache 失效）
- session 間靜默超過 5 分鐘（cache TTL 到期）

**邊界條件**：
- session 若 turn_count < 3，cache_hit_rate 無意義 → `INSUFFICIENT_DATA` error
- 純 subagent session（人工不互動）cache_hit 可能天生低 → 此 case 加入 evidence 說明

---

### 4.3 Correction Spiral（修正螺旋）

**定義**：對同一個檔案反覆修改，且 output token 有遞增趨勢，表示修改品質下降。

| 訊號 | 閾值 | 方向 |
|------|------|------|
| `repeated_edit_peak` | ≥ 4 | equal / above |
| `output_token_ratio` | > 0.40 | above |

**組合規則**：兩個條件**同時滿足**才分類為此 pattern。

**Severity**：
- 先滿足 warn 分類條件（兩者同時成立）
- 若此時 `repeated_edit_peak` ≥ 8 OR `output_token_ratio` > 0.60 → `alert`
- 其他（滿足分類條件）→ `warn`

**常見原因**：
- context window 中 Claude 無法看到完整檔案，每次只修一部分
- 需求頻繁變更
- Claude 未善用 diff-only 輸出

**邊界條件**：
- 同一檔案重複讀取（Read tool）不計入 `repeated_edit_peak`，只計 `Edit` / `Write` / `MultiEdit`
- `output_token_ratio` 計算使用整個 session 的聚合值（非逐 turn 趨勢，簡化實作）

---

### 4.4 Subagent Swarm（子代理海嘯）

**定義**：單一 session 中啟動過多 subagent，協調開銷大、token 效率低。

| 訊號 | 閾值 | 方向 |
|------|------|------|
| `subagent_count` | > 10 | above |

**Severity**：
- `subagent_count` > 20 → `alert`
- `subagent_count` ∈ (10, 20] → `warn`

**常見原因**：
- 一個 task 過度拆分到 subagent
- 每次工具呼叫都啟新 agent（誤解 subagent 用途）

**邊界條件**：
- `subagent_count` 的計算方式：計 `Agent` tool 呼叫次數
- 若 JSONL 缺少 tool use 記錄（老版本）→ `subagent_count` = 0，不觸發此分類

---

### 4.5 Kitchen Sink（大雜燴）

**定義**：session 內話題頻繁跳轉，且重複讀取同一檔案，表示 session 沒有聚焦。

| 訊號 | 閾值 | 方向 |
|------|------|------|
| `topic_shift_count` | > 3 | above |
| `repeated_edit_peak` | ≥ 2 | above（補充訊號，非必要）|

**組合規則**：`topic_shift_count > 3` 是**主要判斷**；`repeated_edit_peak` 作為次要訊號列入 evidence。

**Severity**：
- `topic_shift_count` > 6 → `warn`
- `topic_shift_count` ∈ (3, 6] → `info`

**常見原因**：
- 使用者在一個 session 內處理多個不相關任務
- 「順便問一下」累積成大量話題跳轉

**邊界條件**：
- `topic_shift_count` 的計算見 Section 4.7
- Kitchen Sink 是三個 Anti-pattern 中 severity 最低的，通常是 `info`

---

### 4.6 Marathon Session（馬拉松模式）

**定義**：長時間、高輪數、cache 保溫良好的深度工作模式，是**正常但值得標記**的模式。

| 訊號 | 閾值 | 方向 |
|------|------|------|
| `turn_count` | ≥ 100 | equal / above |
| `duration_minutes` | ≥ 120 | equal / above |
| `cache_hit_rate` | ≥ 0.70 | equal / above |

**組合規則**：三個條件**至少兩個**滿足才分類為此 pattern。

**Severity**：`info`（正常模式，無需警示）

**常見原因**：
- 大型重構或功能開發
- 與 Claude 進行深度設計討論

**邊界條件**：
- 若 Marathon 同時觸發 Correction Spiral（`repeated_edit_peak` ≥ 4 + `output_token_ratio` > 0.40），Correction Spiral 優先
- `duration_minutes` 從 JSONL 第一條 timestamp 到最後一條 timestamp 計算

---

### 4.7 Observer Session（觀察者模式）

**定義**：輕量偵察 session，以閱讀/搜尋為主，幾乎沒有修改操作。

| 訊號 | 閾值 | 方向 |
|------|------|------|
| `turn_count` | < 20 | below |
| `repeated_edit_peak` | ≤ 1 | below / equal |

**組合規則**：兩個條件**同時滿足**。

**Severity**：`info`（正常模式）

**常見原因**：
- 探索新 codebase
- 偵察前置調查

---

### 4.8 Normal Session（正常模式）

**定義**：不符合以上任何 pattern 的 session。

**Severity**：`info`

**Evidence**：空 list（無觸發訊號）

---

### 4.9 Topic Shift Count 計算 Heuristic（v0.1）

> **重要**：`topic_shift_count` 是近似估算，**不保證精確**。設計目標是「大致正確」，而非完美。

**計算方法（v0.1 簡化版）**：

**v0.1 實作方案**：用 assistant turn 的工具呼叫模式作為代理訊號（proxy）：
- 前 2 個 assistant turns 都只使用搜尋工具（`Read`, `Grep`, `Glob`）
- 當前 assistant turn 首次出現編輯工具（`Edit`, `Write`, `MultiEdit`）
- 每次符合上述轉換，算一次話題切換

**校準計劃**：v0.2 版本加入語義相似度（用 embedding 或關鍵詞 cosine distance）。

---

### 4.10 Pattern 分類閾值總覽（實作時的 const 定義）

```rust
// mcp-server/src/pattern_classifier.rs 的常數區域

// Cold Session
pub const COLD_SESSION_CACHE_HIT_WARN: f64 = 0.30;
pub const COLD_SESSION_CACHE_HIT_ALERT: f64 = 0.15;

// Correction Spiral
pub const CORRECTION_SPIRAL_EDIT_PEAK_WARN: u32 = 4;
pub const CORRECTION_SPIRAL_OUTPUT_RATIO_WARN: f64 = 0.40;
pub const CORRECTION_SPIRAL_EDIT_PEAK_ALERT: u32 = 8;
pub const CORRECTION_SPIRAL_OUTPUT_RATIO_ALERT: f64 = 0.60;

// Subagent Swarm
pub const SUBAGENT_SWARM_COUNT_WARN: u32 = 10;
pub const SUBAGENT_SWARM_COUNT_ALERT: u32 = 20;

// Kitchen Sink
pub const KITCHEN_SINK_TOPIC_SHIFT_INFO: u32 = 3;
pub const KITCHEN_SINK_TOPIC_SHIFT_WARN: u32 = 6;

// Marathon
pub const MARATHON_TURN_COUNT: u32 = 100;
pub const MARATHON_DURATION_MIN: u32 = 120;
pub const MARATHON_CACHE_HIT: f64 = 0.70;

// Observer
pub const OBSERVER_MAX_TURNS: u32 = 20;
pub const OBSERVER_MAX_EDIT_PEAK: u32 = 1;

// Minimum turns for meaningful classification
pub const MIN_TURNS_FOR_CLASSIFICATION: u32 = 3;
```

---

## 5. Severity 決定規則

| Severity | 意義 | 典型 Pattern |
|----------|------|--------------|
| `info` | 資訊性，無需立即行動 | Marathon, Observer, Normal, mild Kitchen Sink |
| `warn` | 注意，建議調整工作流 | Cold Session (0.15-0.30), Correction Spiral, mid Subagent Swarm |
| `alert` | 緊急，明顯效率損失 | Cold Session (<0.15), severe Correction Spiral, Subagent Swarm >20 |

---

## 6. 邊界條件（Boundary Conditions）

| 情況 | 處理方式 |
|------|---------|
| 空 session（0 turns） | 返回 `INSUFFICIENT_DATA` error |
| 單 turn session（1-2 assistant turns） | 返回 `INSUFFICIENT_DATA` error |
| 純 subagent session（主 agent 無互動） | 仍可分類，不額外注入隱藏欄位 |
| 跨日 session（midnight crossing） | 以 JSONL 實際 timestamp 差計算 duration，不受日期影響 |
| 缺少 timestamp（老版 JSONL） | `duration_minutes` = null，跳過依賴此訊號的分類條件 |
| 缺少 tool use records | `subagent_count` = 0，`repeated_edit_peak` = 0 |
| session_id 前綴衝突（多個 session 前綴相同）| 返回 `AMBIGUOUS_SESSION_ID`，要求提供更長的 ID |

---

## 7. 資料來源對應（Signal Source Mapping）

每個訊號的資料來源（對應 CTA 現有資料模型）：

| 訊號 | 來源 | Rust struct/方法 |
|------|------|-----------------|
| `cache_hit_rate` | pattern_signals.rs → ParseResult totals | `cache_read / (input + cache_read + cache_creation)` |
| `output_token_ratio` | pattern_signals.rs → ParseResult totals | `output_tokens / (input_tokens + output_tokens)` |
| `subagent_count` | pattern_signals.rs → parser tool records | `Agent` tool use count |
| `repeated_edit_peak` | pattern_signals.rs → parser tool records | 按 file path 分組的 `Edit`/`Write`/`MultiEdit` max count |
| `turn_count` | pattern_signals.rs → ParseResult totals | `parse_result.total_turns` |
| `duration_minutes` | pattern_signals.rs → parser timestamps | first_timestamp to last_timestamp |
| `topic_shift_count` | pattern_signals.rs → tool use patterns | v0.1: search-only → edit heuristic |

**實作邊界**
- `SessionAnalysis` 保留既有聚合責任。
- `pattern_signals.rs` 是 `ParseResult -> Signals` 的薄 seam。
- `classify_session_pattern` MCP handler 消費 `Signals`，不把 usage-pattern 專用欄位反向塞進 `SessionAnalysis`。

---

## 8. 整合點（Integration Points）

### 8.1 與現有 MCP Tools 的關係

```
analyze_session          → 既有參考路徑（ParseResult + SessionAnalysis）
classify_session_pattern → 直接讀 JSONL 並結合 parser/signal builder/classifier
sync_db                  → skill 層 freshness step，不是此 MCP tool 的硬依賴
```

### 8.2 Skill 執行流程

```
使用者輸入「分析 session abc12345 的使用模式」
    ↓
cta router 路由到 cta-usage-pattern
    ↓
若使用者要求最新資料，先 sync_db
    ↓
classify_session_pattern { session_id: "abc12345" }
    ↓ PatternResult JSON
LLM 合成建議報告
    + 引用 harness-signals-to-advice.md
    + 生成 ASCII sparkline（trend data）
    ↓
輸出：分類標籤 + 訊號值 + 建議 + 視覺化
```

---

## 9. 驗收標準（Acceptance Criteria）

### 9.1 功能驗收

| # | 驗收項目 | 驗證方式 |
|---|---------|---------|
| AC-1 | `classify_session_pattern` 正確分類 marathon session | MCP handler unit test with synthetic JSONL fixture |
| AC-2 | `classify_session_pattern` 正確分類 correction_spiral | MCP handler unit test with synthetic JSONL fixture |
| AC-3 | `classify_session_pattern` 正確分類 cold_session | MCP handler unit test with synthetic JSONL fixture |
| AC-4 | 空 session 返回 INSUFFICIENT_DATA error | Unit test |
| AC-5 | session_id 不存在返回 SESSION_NOT_FOUND error | Unit test |
| AC-6 | 多個 pattern 條件滿足時，優先順序正確（Cold > Spiral > ...）| Unit test |
| AC-7 | evidence list 包含所有觸發條件的具體數值 | Unit test |
| AC-8 | `cta-usage-pattern` skill 與 advice mapping 文件已對齊 MCP-8 | Mapping/doc test |

### 9.2 品質驗收

| # | 驗收項目 | 驗證方式 |
|---|---------|---------|
| QA-1 | `cargo test --all-targets` 全綠 | CI |
| QA-2 | `cargo clippy -- -D warnings` 無 warning | CI |
| QA-3 | 無新增 hardcode（pattern 閾值都在 const 區域）| Code review |
| QA-4 | Pattern classifier / signal / MCP-8 tests 覆蓋所有分類與主要邊界 | `cargo test` |
| QA-5 | 重構後 API surface 不變（外部 crate 用法不受影響）| Compilation test |

---

## 10. 開放問題（Open Questions）

| ID | 問題 | 狀態 | 負責人 |
|----|------|------|--------|
| OQ-1 | 初始閾值（如 `repeated_edit_peak ≥ 4`）是否需要使用者調整？ | 暫不支援，v0.2 加 config | ADR-0002 |
| OQ-2 | `topic_shift_count` v0.1 heuristic 的 precision/recall 如何測量？ | 待 golden set 建立後測量 | 後續工作 |
