---
name: cta-project-review
description: |
  This skill should be used when the user asks to "analyze this project", "project health check",
  "分析這個專案", "專案健檢", "subagent efficiency", "tool usage analysis", "工具使用",
  or needs detailed analysis of a specific project's token usage patterns. Provides four-dimension
  analysis: cost, efficiency, tools, and architecture. Can also be routed from the main cta skill.
---

# CTA Project Review — Four-Dimension Project Analysis

Deep analysis of a specific project across cost, efficiency, tool usage, and architecture dimensions.

## Workflow

### Step 1: Sync Data
Execute `mcp__token-analyzer__sync_db`. Skip if already called in this conversation.

### Step 2: Identify Project
If user specifies project_path, use it directly.
Otherwise, execute `mcp__token-analyzer__analyze_global` and present the project ranking for user to choose.

### Step 3: Analyze Project
Execute `mcp__token-analyzer__analyze_project` with:
- `project_path`: the identified project
- `include_subagents`: true
- `sort_by`: "cost"
- `limit`: 20

### Step 4: Output Four-Dimension Report

```markdown
## CTA 專案分析 — project-name

### 成本面
| 指標 | 值 |
|------|-----|
| 總會話數 | N |
| 總成本 | $X.XX USD |
| 平均每會話 | $X.XX |
| Top 3 最貴會話 | a1b2c3d4($X), e5f6g7h8($X), ... |

### 效率面
| 指標 | 值 |
|------|-----|
| 平均 Cache 命中率 | X.X% |
| 低 cache 會話比例 | X.X% |

### 工具面
| 工具 | 總調用 | 使用會話數 | 每會話平均 |
|------|--------|-----------|-----------|
| Read | X | N | X.X |
| Bash | X | N | X.X |
| Agent | X | N | X.X |

### 架構面
| 指標 | 值 |
|------|-----|
| Main 會話數 | N |
| Subagent 會話數 | N |
| Subagent Token 佔比 | X.X% |
| Subagent Overhead | $X.XX (估算) |
| 模型分布 | Opus X% / Sonnet X% / Haiku X% |
```

### Step 5 (Optional): Subagent Overhead
To calculate overhead, execute `mcp__token-analyzer__analyze_project` again with `include_subagents=false`.
Overhead = (cost with subagents) - (cost without subagents).

### Step 6 (Optional): Session Drill-Down
For top 3 most expensive sessions, execute `mcp__token-analyzer__analyze_session` with each session_id.

## Behavior Rules

1. If no project_path given, show project ranking from analyze_global and let user choose.
2. Calculate "per session average" as total_invocations / session_count for tool ranking.
3. High Agent tool usage often correlates with high subagent ratio — cross-validate architecture dimension.
4. Cache hit rate <70%: mark with warning. Subagent ratio >20%: mark with notice.

## Output Rules

- Use 繁體中文 for prose, English for technical terms.
- Currency: `$X.XX USD`.
- Percentages: one decimal place.
- session_id: first 8 characters only.
- Token counts: thousands separator (`125,000`).

## Additional Resources

For MCP tool parameter details: `../cta/references/tool-reference.md`
