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

## Output Language

Configured output language: `${user_config.output_language}`.

- `en`: use English for human-readable prose, headings, and table labels.
- `zh-TW`: use Traditional Chinese for human-readable prose, headings, and table labels.
- `auto`, unset, empty, unsupported values, or a literal unexpanded placeholder: follow the latest user message's primary natural language; if that is unclear, fall back to English.
- Keep technical identifiers such as metric names, tool names, pattern IDs, JSON fields, and session IDs in English.

## Workflow

### Step 1: Sync Data
Execute `mcp__token-analyzer__sync_db`. Skip if already called in this conversation.

### Step 2: Global Analysis
Execute `mcp__token-analyzer__analyze_global` with no parameters.

### Step 3: Output Summary
Format results as the following table. Fill every row from the analyze_global response.
Localize every human-readable label and sentence; the English example below defines structure only.

```markdown
## CTA Health Report

| Metric | Value |
|--------|-------|
| Total Sessions | X |
| Total Projects | X |
| Total Cost | $X.XX USD |
| Average Cache Hit Rate | X.X% |
| Subagent Token Ratio | X.X% |

### Top 3 Projects by Cost
1. project-name — $X.XX (N sessions)
2. ...
3. ...

### Top 3 Sessions by Cost
1. a1b2c3d4 — $X.XX (project-name)
2. ...
3. ...
```

### Step 4: Ask Direction
After presenting the summary, ask:
Ask which area the user wants to explore next: cost, anomalies, projects, or trends. Localize the question using the Output Language contract.

Route the user's choice to the corresponding sub-skill:

| Choice | Invoke |
|--------|--------|
| Cost / 成本 | `cta-cost-audit` |
| Anomalies / 異常 | `cta-anomaly-hunt` |
| Projects / 專案 | `cta-project-review` |
| Trends / 趨勢 | `cta-trend-watch` |

## Output Rules

- Currency: `$X.XX USD`.
- Percentages: one decimal place (`85.3%`).
- session_id: first 8 characters only (`a1b2c3d4`).
- Token counts: thousands separator (`125,000`).
- Cache hit rate < 70%: mark with warning.
- Subagent ratio > 20%: mark with notice.

## Additional Resources

For MCP tool parameter details: `${CLAUDE_PLUGIN_ROOT}/skills/cta/references/tool-reference.md`
