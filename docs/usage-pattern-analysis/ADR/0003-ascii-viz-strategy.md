# ADR-0003: ASCII 視覺化策略 — 自寫 Sparkline + chartli Optional Feature

> **狀態**：已接受（Accepted）
> **日期**：2026-04-14
> **決策者**：Yue Li（repo owner）+ 技術調研

---

## 情境（Context）

`cta-usage-pattern` skill 的報告輸出需要 ASCII 視覺化，讓使用者一眼辨識趨勢（token 消耗趨勢、cache hit rate 趨勢等）。

需要決定：
1. 使用哪個 ASCII 圖表庫
2. 如何整合到現有 CTA 架構
3. 是否作為可選功能

---

## 評估的選項

### 選項 A：`textplots` crate

- 特性：純 line plot，極輕量
- 缺點：只有折線圖；API 較陳舊（2022 年最後更新）
- 評估：功能不足，不選

### 選項 B：`plotters` crate

- 特性：功能豐富，支援多種輸出格式
- 缺點：主要面向 raster 圖形輸出（PNG、SVG），ASCII 非主力用途；dependency 重（需要 freetype、image 等）
- 評估：過重，不適合 CLI 工具，不選

### 選項 C：`chartli` crate（2026-03 更新）

- 特性：terminal-first，支援 ascii/unicode/braille/spark/bar/column/heatmap
- 優點：專為 terminal 設計，API 簡潔
- 缺點：相對新的 crate（維護狀態需長期觀察）
- 評估：好選，但不作為必要依賴

### 選項 D：自寫 Sparkline（本決策選擇 + 主要依賴）

- 特性：< 50 行 Rust，使用 block characters（`▁▂▃▄▅▆▇█`）
- 優點：零外部依賴、完全可控、terminal 兼容性最穩
- 缺點：功能簡單（只有 sparkline）

---

## 決策（Decision）

採用**雙層策略**：

1. **自寫 Sparkline 為 baseline（必要）**：
   - 在 `mcp-server/src/` 新增 `sparkline.rs`（< 50 行）
   - 實作 `pub fn render(data: &[f64]) -> String`
   - 使用 Unicode block characters：`▁▂▃▄▅▆▇█`
   - 零外部依賴
   - 在 CLI `cta trend` 和 skill 輸出中使用

2. **`chartli` 為 Optional Feature（選用）**：
   - 透過 Cargo feature flag：`features = ["ascii-viz"]`
   - `Cargo.toml`：`chartli = { version = "X.Y", optional = true, features = ["ascii"] }`
   - 啟用後可使用更豐富的 bar chart、heatmap
   - 默認不啟用（不增加 binary 大小）

---

## 實作規格

### Sparkline 自寫實作（`src/sparkline.rs`）

```rust
// 目標 API
pub fn render(data: &[f64]) -> String

// 工作原理
// 1. 找 data 的 min 和 max
// 2. 把每個值映射到 0-7（8 個等級）
// 3. 對應 BLOCKS 陣列輸出字元

const BLOCKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

// 邊界處理
// - data 為空 → 返回空字串
// - data 全為相同值 → 所有格顯示 '▄'（中間值）
// - 包含 NaN → 用 '·' 表示缺失
```

### Skill 輸出示例

```
Token 趨勢（過去 14 天）
▁▂▄▆█▇▅▃▂▃▅▇▆▄
04-01           04-14

Cache Hit Rate
▆▇█▇▆▅▄▃▂▁▂▃▄▅
70%             45%（⚠ 下降趨勢）
```

---

## 理由（Rationale）

### 為何自寫而非直接用 chartli？

| 考量 | 自寫 | chartli |
|------|------|---------|
| 外部依賴 | 零 | 新增一個 crate |
| binary 大小 | 無影響 | 增加 ~50KB |
| terminal 兼容性 | 最高（只用 Unicode block）| 高，但 braille 在某些 terminal 顯示有問題 |
| 維護風險 | 無（自己的程式碼）| 低（crate 可能停更）|
| 功能 | 只有 sparkline | bar/heatmap/column 等多種 |

**結論**：對 CTA 的需求（展示 token 趨勢），sparkline 已足夠。自寫 < 50 行比引入外部依賴更合理。

### 為何還要提供 chartli optional feature？

部分使用者可能想要 heatmap（例如：一週中每小時的 token 使用熱圖）。提供 optional feature 讓進階使用者可以選擇，不強制增加所有使用者的 binary 大小。

---

## 測試策略

| 測試 | 內容 |
|------|------|
| `sparkline::render` 單元測試 | 空陣列、全相同值、正常陣列、含 NaN |
| Golden output 測試 | 固定輸入 → 固定輸出字串（snapshot test）|
| Terminal 寬度邊界測試 | 超過 80 字元時自動截斷或換行 |

---

## 後果（Consequences）

### 正面
- 零外部依賴（baseline）
- Binary 大小不變
- 完全可控，不受 crate 棄用影響

### 負面
- Sparkline 只是一維折線，無法展示多維資料
- 需要自行維護 sparkline 邏輯（雖然 < 50 行）

### 緩解
- Optional feature 提供未來擴展路徑
- Sparkline 邏輯極簡，維護成本接近零

---

## 審查觸發條件

1. 使用者要求 heatmap 或多維視覺化
2. chartli 達到 1.0 穩定版，值得升為預設依賴
3. sparkline 自寫邏輯有 bug 導致輸出不正確

---

## 相關文件

- [SPEC.md](../SPEC.md) — 分析工具整體規格
- [ADR-0001](0001-hybrid-mcp-skill.md) — skill 層負責呼叫 sparkline 渲染
