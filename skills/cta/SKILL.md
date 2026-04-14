---
name: cta
description: |
  This skill should be used when the user asks to "analyze token usage", "check costs",
  "how much did I spend", "token analysis", "session analysis", "cta", or mentions
  Claude Code usage efficiency, token spending, or cost optimization.
  Routes to 6 workflow sub-skills: cta-health-check, cta-cost-audit, cta-anomaly-hunt,
  cta-project-review, cta-trend-watch, cta-usage-pattern. Also handles ambiguous intent with a quick overview.
  Do NOT trigger for: building token-related features, tokenizer/NLP work, or non-Claude cost analysis.
---

# CTA — Claude Token Analyzer Router

Route token analysis requests to the appropriate workflow skill, or provide a quick overview for ambiguous intent.

## Prerequisites

1. **sync_db when freshness matters** — Execute `mcp__token-analyzer__sync_db` before workflows that read SQLite-backed aggregates (`analyze_global`, `analyze_project`, `cost_report`, `anomaly_scan`, `trend_report`) when the user asks for latest data. Historical `classify_session_pattern` lookups can run directly against JSONL without waiting for `sync_db`.
2. **Output language** — Use 繁體中文 for prose. Keep English for technical terms (cache_hit_rate, session_id, subagent, tool names).
3. **Error handling** — Report MCP tool errors to the user; never swallow silently. State explicitly when results are empty.

## Intent Routing

| User Intent | Keywords | Route to |
|-------------|----------|----------|
| Quick status | 「看看」「總覽」「花了多少」「現在狀況」 | `cta-health-check` |
| Monthly cost | 「這個月」「成本報告」「月度」「預算」 | `cta-cost-audit` |
| Find anomalies | 「異常」「有問題嗎」「診斷」「排查」 | `cta-anomaly-hunt` |
| Project analysis | 「分析專案」「專案健檢」「subagent」「工具使用」 | `cta-project-review` |
| Trend forecast | 「趨勢」「在漲嗎」「預測」「燃率」 | `cta-trend-watch` |
| Usage pattern | 「使用模式」「pattern 分析」「harness 優化」「工作流建議」「sparkline」 | `cta-usage-pattern` |

### Routing Behavior

1. **Clear intent** — Invoke the matching sub-skill directly.
2. **Ambiguous intent** (「幫我看看 token」「分析一下」) — If latest data matters, run sync_db → analyze_global → output a one-page summary (format below) → ask which direction to explore.
3. **Cross-domain** — Sub-skills may route to each other mid-workflow.

## Shared Output Format

| Element | Format |
|---------|--------|
| Currency | `$X.XX USD` |
| Percentage | One decimal (`85.3%`) |
| session_id | First 8 chars (`a1b2c3d4`) |
| Token count | Thousands separator (`125,000`) |
| Large output (>50K chars) | Write to `${TMPDIR:-/tmp}/cta-*.md`, report path |
| Cache hit rate <70% | Mark with warning |
| Subagent ratio >20% | Mark with notice |

## One-Page Summary Template (for ambiguous intent)

```markdown
## CTA Overview

| Metric | Value |
|--------|-------|
| Total Sessions | X |
| Total Projects | X |
| Total Cost | $X.XX USD |
| Avg Cache Hit Rate | X.X% |
| Subagent Token Ratio | X.X% |

### Top 3 Projects (by cost)
1. project-name — $X.XX (N sessions)

### Top 3 Sessions (by cost)
1. a1b2c3d4 — $X.XX (project-name)
```

Follow with: 「要深入哪個方向？成本 / 異常 / 專案 / 趨勢」

## Additional Resources

### Reference Files

For complete MCP tool parameters, return types, and known caveats:
- **`references/tool-reference.md`** — 8 MCP tool quick reference with parameter defaults, advanced tuning, and known pitfalls

Sub-skills reference this file at the absolute path `${CLAUDE_PLUGIN_ROOT}/skills/cta/references/tool-reference.md`.
