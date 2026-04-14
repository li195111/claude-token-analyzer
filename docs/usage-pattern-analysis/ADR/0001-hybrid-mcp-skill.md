# ADR-0001: 混合架構 — 薄 Rust MCP tool + LLM Skill 層合成

> **狀態**：已接受（Accepted）
> **日期**：2026-04-14
> **決策者**：Yue Li（repo owner）+ 架構分析

---

## 情境（Context）

需要實作 `classify_session_pattern` 功能，讓 CTA 能回答「這個 session 是什麼使用模式？」並給出 Harness Engineer 優化建議。

在架構設計時有三個候選方案：

**方案 A — 純 LLM（無新 Rust code）**：
- skill 直接呼叫現有 `analyze_session` MCP tool，由 LLM 依數值自行判斷 pattern

**方案 B — 純 Rust（所有邏輯在 backend）**：
- Rust 實作分類器 + 建議文字生成器，直接返回完整報告

**方案 C — 混合架構（本決策選擇）**：
- Rust 實作薄分類器：計算硬訊號、輸出結構化 PatternResult
- skill 層 LLM：接收 PatternResult，合成建議文字 + ASCII 視覺化

---

## 決策（Decision）

採用**方案 C — 混合架構**。

新增 Rust MCP tool `classify_session_pattern`：
- 只負責計算硬訊號（cache_hit_rate、repeated_edit_peak 等）
- 應用明確的分類規則（const 閾值）
- 輸出結構化 JSON（PatternResult）

skill `cta-usage-pattern` 層（LLM）負責：
- 呼叫 `classify_session_pattern` 取得硬訊號
- 依 `harness-signals-to-advice.md` 對應表合成建議文字
- 生成 ASCII sparkline 視覺化
- 整合成人類可讀報告

---

## 理由（Rationale）

### 拒絕方案 A（純 LLM）

| 問題 | 說明 |
|------|------|
| 不可測試 | LLM 判斷是 non-deterministic，無法寫 golden set 斷言 |
| 不可驗證 | 無法在 CI 中確認分類邏輯正確性 |
| 成本浪費 | 每次分析都要把原始 JSON 塞進 LLM context，token 成本高 |
| 不一致 | 同樣數值在不同 session 中可能得到不同分類 |

### 拒絕方案 B（純 Rust）

| 問題 | 說明 |
|------|------|
| 建議文字呆板 | Rust 生成的固定文字缺乏語境感知 |
| 維護成本高 | 每新增一種建議都要修改 Rust code 並重新發布 binary |
| 語言靈活性差 | 無法依使用者語言（中文/英文）調整建議文字 |
| 違反職責分離 | Rust binary 不應承擔 "presentation layer" 邏輯 |

### 選擇方案 C（混合）的理由

1. **硬訊號可測試**：`cache_hit_rate < 0.30 → cold_session` 是確定性規則，可寫 golden set + assert

2. **建議文字靈活**：LLM 可依上下文、語言偏好、session 特性合成自然語言建議

3. **符合 Anthropic 「brain 與 hands 解耦」原則**：Rust 做數值計算（hands），LLM 做合成與判斷（brain）

4. **符合 CTA 現有架構模式**：現有 7 個 MCP tools 都是「計算 → 結構化 JSON」，skill 再呼叫多個 tool 合成報告

5. **分類器可獨立演化**：閾值調整、新增 pattern 只改 Rust；建議文字改進只改 skill

---

## 後果（Consequences）

### 正面
- 分類邏輯完全可測試（CI green = 分類正確）
- 建議文字品質由 LLM 保證，隨模型升級自然提升
- 新增 pattern 只需改 `pattern_classifier.rs` + `pattern-definitions.md`，不需改 skill 文字

### 負面
- 需要兩個開發層（Rust + skill SSOT）保持一致
- PatternResult JSON schema 需要版本管理（未來 skill 更新需相容舊 JSON 格式）
- skill 層的建議文字品質依賴 LLM，難以量化驗收

### 緩解
- FEATURE-REGISTRY.md 追蹤 MCP-8 和 SK-7 的一致性
- SPEC.md 定義 PatternResult 的穩定 schema，schema 變更需要版本號

---

## 替代方案考慮（詳細）

### 為何不在 skill 層用 `analyze_session` 原始 JSON 直接分類？

`analyze_session` 的輸出已包含 `cache_hit_rate`、`output_tokens` 等欄位。從技術上 LLM 可以直接讀取這些數值自行判斷 pattern。

**問題**：
1. `analyze_session` 不包含 `repeated_edit_peak`、`subagent_count`、`topic_shift_count` 等新訊號——需要新增欄位
2. 即使有欄位，讓 LLM 每次重新「發明」分類規則 = 不確定性太高
3. skill 層會累積 case-by-case 調試負擔（pattern A 正確但 pattern B 不對時，怎麼修？）

**結論**：新增薄 MCP tool 是必要的，不是過度設計。

---

## 相關文件

- [SPEC.md](../SPEC.md) — `classify_session_pattern` 的完整輸入/輸出合約
- [ADR-0002](0002-pattern-classification-thresholds.md) — 閾值初始值來源與校準計劃
- [FEATURE-REGISTRY.md](../FEATURE-REGISTRY.md) — MCP-8 和 SK-7 的追蹤記錄
