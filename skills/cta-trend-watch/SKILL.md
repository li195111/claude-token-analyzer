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

## Output Language

Configured output language: `${user_config.output_language}`.

- `en`: write all human-readable prose, headings, and table labels in English, even when the user writes in another language. The configured value overrides the language of the user's message.
- `zh-TW`: write all human-readable prose, headings, and table labels in Traditional Chinese, even when the user writes in another language. The configured value overrides the language of the user's message.
- `auto`, unset, empty, unsupported values, or a literal unexpanded placeholder: follow the latest user message's primary natural language; if that is unclear, fall back to English.
- Keep technical identifiers such as metric names, tool names, pattern IDs, JSON fields, and session IDs in English.
- Before sending the final response, confirm its prose language matches this contract; if it does not, rewrite it in the required language first.

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

Localize every human-readable label and sentence; the English example below defines structure only.

```markdown
## CTA Trend Report — Last N Days

| Metric | Value |
|--------|-------|
| Average Daily Cost | $X.XX USD |
| Average Daily Tokens | X |
| Peak Day | YYYY-MM-DD ($X.XX) |
| Recent 7-Day Average | $X.XX USD |
| Previous 7-Day Average | $X.XX USD |
| Trend Direction | ↑ +X.X% / ↓ -X.X% |
| Monthly Projection | $X.XX USD |

### Trend Analysis
- (Describe trend: stable / rising / declining based on data)
- (If rising >20%: warn and suggest cta-anomaly-hunt)

### Daily Detail
| Date | Cost | Tokens | Sessions |
|------|------|--------|----------|
| ... | ... | ... | ... |
```

### Step 5 (Conditional): Trend Alert
If the 7-day trend shows >20% increase, proactively suggest:
State in the configured language that the trend increased materially and suggest `cta-anomaly-hunt` to investigate.

## Behavior Rules

1. Default to daily granularity + 30 days. Accept weekly/monthly and custom day ranges.
2. Flag trends exceeding +20% as warnings and suggest anomaly investigation.
3. Monthly projection = `avg_daily_cost * total days in month` (not remaining days).
4. Support per-project filtering via `project_path` parameter.
5. When fewer than 14 data points exist, skip 7-day comparison and note insufficient data.

## Output Rules

- Currency: `$X.XX USD`.
- Percentages: one decimal place (`+15.3%`).
- Token counts: thousands separator (`125,000`).
- Trend arrows: ↑ for increase, ↓ for decrease, → for stable (< 3%).

## Additional Resources

For MCP tool parameter details: `${CLAUDE_PLUGIN_ROOT}/skills/cta/references/tool-reference.md`
