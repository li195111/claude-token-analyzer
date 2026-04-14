---
name: cta-health-check
description: |
  This skill should be used when the user asks for a "quick check", "overview",
  "how much did I spend", "看看狀況", "總覽", or wants a fast one-page summary
  of Claude Code token usage and costs. The lightest CTA workflow, completes in
  under 3 minutes. Can also be routed from the main cta skill.
---

# CTA Health Check — Quick Overview

One-page summary of Claude Code usage status. The lightest CTA workflow.

## Workflow

### Step 1: Sync Data
Execute `mcp__token-analyzer__sync_db`. Skip if already called in this conversation.

### Step 2: Global Analysis
Execute `mcp__token-analyzer__analyze_global` with no parameters.

### Step 3: Output Summary
Format results as the following table. Fill every row from the analyze_global response.

```markdown
## CTA 健檢報告

| 指標 | 值 |
|------|-----|
| 總會話數 | X |
| 總專案數 | X |
| 總成本 | $X.XX USD |
| 平均 Cache 命中率 | X.X% |
| Subagent Token 佔比 | X.X% |

### Top 3 燒錢專案
1. project-name — $X.XX (N sessions)
2. ...
3. ...

### Top 3 最貴會話
1. a1b2c3d4 — $X.XX (project-name)
2. ...
3. ...
```

### Step 4: Ask Direction
After presenting the summary, ask:
> 「要深入哪個方向？成本 / 異常 / 專案 / 趨勢」

Route the user's choice to the corresponding sub-skill:

| Choice | Invoke |
|--------|--------|
| 成本 | `cta-cost-audit` |
| 異常 | `cta-anomaly-hunt` |
| 專案 | `cta-project-review` |
| 趨勢 | `cta-trend-watch` |

## Output Rules

- Use 繁體中文 for prose, English for technical terms.
- Currency: `$X.XX USD`.
- Percentages: one decimal place (`85.3%`).
- session_id: first 8 characters only (`a1b2c3d4`).
- Token counts: thousands separator (`125,000`).
- Cache hit rate < 70%: mark with warning.
- Subagent ratio > 20%: mark with notice.

## Additional Resources

For MCP tool parameter details: `../cta/references/tool-reference.md`
