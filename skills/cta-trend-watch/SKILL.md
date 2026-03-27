---
name: cta-trend-watch
description: |
  This skill should be used when the user asks about "trends", "usage trends",
  "趨勢", "用量在漲嗎", "預測", "燃率", "burn rate", or wants to understand
  whether Claude Code usage is increasing or decreasing over time. Supports
  daily/weekly/monthly granularity with simple forecasting. Can also be routed
  from the main cta skill.
---

# CTA Trend Watch — Usage Trend Analysis

Analyze token usage and cost trends over time, with derived metrics and simple forecasting.

## Workflow

### Step 1: Sync Data
Execute `mcp__token-analyzer__sync_db`. Skip if already called in this conversation.

### Step 2: Fetch Trends
Execute `mcp__token-analyzer__trend_report` with:
- `granularity`: "daily" (default). Accept "weekly" or "monthly" from user.
- `last_n_days`: 30 (default). Accept custom range from user.
- `project_path`: optional, for per-project filtering.

### Step 3: Calculate Derived Metrics

From the returned `data_points` array, compute:
- **Daily average cost**: `avg_daily_cost` (from API)
- **Daily average tokens**: `avg_daily_tokens` (from API)
- **Peak day**: `peak_day` (from API)
- **Recent 7-day average**: mean of last 7 data points' `total_cost`
- **Previous 7-day average**: mean of data points `[-14:-7]` `total_cost`
- **Trend direction**: `(recent_7d - prev_7d) / prev_7d * 100`
- **Monthly projection**: `avg_daily_cost * total_days_in_current_month`

### Step 4: Output Report

```markdown
## CTA 趨勢報告 — 最近 N 天

| 指標 | 值 |
|------|-----|
| 日均成本 | $X.XX USD |
| 日均 Token | X |
| 峰值日 | YYYY-MM-DD ($X.XX) |
| 近 7 天均值 | $X.XX USD |
| 前 7 天均值 | $X.XX USD |
| 趨勢方向 | ↑ +X.X% / ↓ -X.X% |
| 本月預估 | $X.XX USD |

### 趨勢分析
- (Describe trend: stable / rising / declining based on data)
- (If rising >20%: warn and suggest cta-anomaly-hunt)

### 每日明細
| 日期 | 成本 | Token | 會話數 |
|------|------|-------|--------|
| ... | ... | ... | ... |
```

### Step 5 (Conditional): Trend Alert
If the 7-day trend shows >20% increase, proactively suggest:
> 「趨勢上升幅度較大，建議執行 cta-anomaly-hunt 排查原因。」

## Behavior Rules

1. Default to daily granularity + 30 days. Accept weekly/monthly and custom day ranges.
2. Flag trends exceeding +20% as warnings and suggest anomaly investigation.
3. Monthly projection = `avg_daily_cost * total days in month` (not remaining days).
4. Support per-project filtering via `project_path` parameter.
5. When fewer than 14 data points exist, skip 7-day comparison and note insufficient data.

## Output Rules

- Use 繁體中文 for prose, English for technical terms.
- Currency: `$X.XX USD`.
- Percentages: one decimal place (`+15.3%`).
- Token counts: thousands separator (`125,000`).
- Trend arrows: ↑ for increase, ↓ for decrease, → for stable (< 3%).

## Additional Resources

For MCP tool parameter details: `${CLAUDE_PLUGIN_ROOT}/skills/cta/references/tool-reference.md`
