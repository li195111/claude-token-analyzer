# ADR-0002: Pattern 分類閾值 — 初始值來源與校準計劃

> **狀態**：已接受（Accepted）
> **日期**：2026-04-14
> **決策者**：Yue Li（repo owner）+ Agent-3 研究

---

## 情境（Context）

`classify_session_pattern` 需要數值閾值來判斷 pattern（例如：`cache_hit_rate < 0.30 → cold_session`）。這些閾值影響分類準確率，需要決定：

1. 初始值的來源（研究 vs 實測 vs 任意猜測）
2. 是否讓使用者自訂閾值
3. 未來如何校準

---

## 決策（Decision）

### 初始閾值來源

採用 **Agent-3 研究報告的數值**（2026-04-14 研究合成），並在 SPEC.md 和 `pattern_classifier.rs` 的 const 區域明確標記來源。

初始閾值表（v0.1，來自研究 + Anthropic 官方文件）：

| Threshold | 值 | 來源分類 |
|-----------|-----|---------|
| Cold Session WARN | cache_hit < 0.30 | Anthropic 官方：「cache hit < 50% 連續 2 session 需注意」下調至保守值 |
| Cold Session ALERT | cache_hit < 0.15 | 研究合成：<15% 表示 cache 完全失效 |
| Correction Spiral edit | repeated_edit ≥ 4 | 研究合成：同檔 3 次 edit 仍屬正常，第 4 次開始表示迴圈 |
| Correction Spiral output | output_ratio > 0.40 | Anthropic 官方：「output tokens 占比 > 40% 應讓 Claude 只輸出 diff」|
| Subagent Swarm WARN | subagent > 10 | 研究合成：單 session >10 subagent 協調成本高 |
| Kitchen Sink INFO | topic_shift > 3 | 研究合成：3 次話題跳轉仍屬常見，超過才算失焦 |
| Marathon turns | turn_count > 100 | 研究合成：100+ turns 屬長 session |
| Marathon duration | duration > 120 min | 研究合成：2 小時以上屬馬拉松 |
| Marathon cache | cache_hit > 0.70 | Anthropic 官方：70% cache hit 為良好 |

### 閾值自訂政策（v0.1）

**v0.1 不支援使用者自訂閾值**。

所有閾值以 `pub const` 定義在 `pattern_classifier.rs` 的頂層 const 區域，集中管理，不 hardcode 在函式體內。

### 校準計劃（v0.2）

v0.2 版本加入：
1. 從 `tests/fixtures/golden_sessions/` 讀取人工標記資料集
2. 計算各閾值對應的 precision/recall
3. 透過 `CTA_CLASSIFIER_CONFIG` 環境變數或 TOML 允許覆蓋

---

## 理由（Rationale）

### 為何不從使用者實際數據自動校準？

CTA 是**事後分析工具**，不是 ML 訓練平台。讓工具自動調整自己的分類規則需要：
- 標記資料（人工確認哪些 session 是 anti-pattern）
- 統計顯著性（需要足夠多樣本）
- 版本管理（閾值變更 = 過去的報告可能改變意義）

這些超出 v0.1 範圍。初始研究值「夠用」：寧可有一個可解釋的固定值，也不要一個「自動調整但說不清楚為什麼」的黑盒。

### 為何不用 Issue #2（Support custom anomaly thresholds via config）的方式？

Issue #2 是針對 `detector.rs` 的 `stddev_threshold`。那個 threshold 影響的是**異常偵測**（統計方法）。pattern 分類的閾值是**業務規則**（有明確語意），性質不同。

v0.1 先保持簡單（const），v0.2 再加 config 支援。

### 為何選 v0.1 的這些數值而非別的？

| 閾值決策 | 保守原則 |
|---------|---------|
| cold session < 0.30（非 0.50）| Anthropic 官方說 50% 需注意，但 50% 閾值觸發率太高。保守設為 30% 以減少誤報。|
| correction spiral edit ≥ 4（非 3）| 3 次可能是正常的「改了改了」，4 次才有 spiral 感 |
| kitchen sink topic > 3（非 2）| 2 次話題跳轉很常見，3 次以上才算失焦 |

**設計原則**：寧可 false negative（漏報），不要 false positive（誤報）。
Harness Engineer 看到太多「警告」會降低信任度，不看警告。

---

## 後果（Consequences）

### 正面
- 分類邏輯完全透明（所有閾值都在 `SPEC.md §4.10` 和 const 區域）
- 任何人都可以理解為什麼一個 session 被分類為某個 pattern
- 不需要 ML 基礎設施

### 負面
- 初始閾值可能不適合所有使用者（有人 100 turns 就算長，有人要 200 turns）
- 不能自適應使用者習慣

### 緩解
- evidence list 在每個 PatternResult 中附上具體數值，使用者可自行判斷是否同意分類
- v0.2 計劃的 config 支援可解決自訂需求
- Golden set（Phase 2）建立後能定量評估初始值品質

---

## 審查觸發條件

下列情況應重新審查此 ADR：

1. 使用者回饋說某個分類結果「根本不對」（precision 問題）
2. Golden set precision < 80%（在 Phase 2 測試後確認）
3. v0.2 加入 config 支援時（需要決定 schema）

---

## 相關文件

- [SPEC.md §4.10](../SPEC.md) — 完整閾值常數定義
- [ADR-0001](0001-hybrid-mcp-skill.md) — 架構決策（為何需要 Rust 分類器）
- [HARNESS-BEST-PRACTICES.md](../HARNESS-BEST-PRACTICES.md) — 閾值來源的研究依據（Phase 7 歸檔）
