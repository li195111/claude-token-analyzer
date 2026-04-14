---
name: cta-cost-audit
description: |
  This skill should be used when the user asks about "monthly costs", "cost report",
  "這個月花多少", "成本報告", "月度審計", "預算", or needs to understand cost
  distribution by project, model, or time period. Supports cross-month comparison
  and model cost optimization suggestions. Can also be routed from the main cta skill.
---

# CTA Cost Audit — Monthly Cost Report

Generate structured monthly cost reports with daily breakdown, project breakdown, and model cost comparison.

## Workflow

### Step 1: Sync Data

Execute `mcp__token-analyzer__sync_db`. Skip if already called in this conversation.

### Step 2: Generate Report

Execute `mcp__token-analyzer__cost_report` with:
- `month`: user-specified YYYY-MM, or current month by default
- `daily`: true
- `project_path`: optional, only if user specifies a project

### Step 3: Output Report

Format results into the following structure. Fill every section from the cost_report response.

```markdown
## CTA 月度成本報告 — YYYY-MM

**月度總成本：$X.XX USD**

### 每日成本
| 日期 | 成本 | 會話數 | 備註 |
|------|------|--------|------|
| 03-01 | $X.XX | N | |
| 03-05 | $X.XX | N | <- peak day |

### 按專案分解
| 專案 | 成本 | 佔比 |
|------|------|------|
| project-a | $X.XX | XX.X% |

### 按模型分解
| 模型 | 成本 | Token 數 | 每百萬 Token 均價 |
|------|------|----------|------------------|
| claude-opus-4-6 | $X.XX | X | $X.XX |
| claude-sonnet-4-6 | $X.XX | X | $X.XX |
| claude-haiku-4-5 | $X.XX | X | $X.XX |

### 優化建議
- (Calculate savings if Opus usage were replaced by Sonnet where applicable)
```

### Step 4 (Optional): Drill Down

If the user asks about a specific project, execute `mcp__token-analyzer__analyze_project` with that project_path.

### Step 5 (Optional): Cross-Month Comparison

If the user requests comparison, call `cost_report` for the previous month and calculate:
- Month-over-month cost change (absolute and percentage)
- Which projects drove the change

## Behavior Rules

1. Default to current month. Accept YYYY-MM format for historical months.
2. Mark peak days automatically in the daily breakdown.
3. Include "per million token" average in model breakdown for cost comparison.
4. If Opus accounts for >50% of cost, proactively suggest evaluating Sonnet for applicable tasks.
5. For cross-month comparison, show delta as both absolute (`$X.XX`) and percentage (`+X.X%`).

## Output Rules

- Use 繁體中文 for prose, English for technical terms.
- Currency: `$X.XX USD`.
- Percentages: one decimal place (`85.3%`).
- Token counts: thousands separator (`125,000`).
- Large output (>50K chars): write to `${TMPDIR:-/tmp}/cta-cost-report.md`, report path.

## Additional Resources

For MCP tool parameter details: `../cta/references/tool-reference.md`
