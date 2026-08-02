---
name: cta-usage-pattern
description: |
  This skill should be used when the user asks about "usage pattern", "pattern analysis",
  "harness optimization", "workflow advice", "ASCII chart", "使用模式", "pattern 分析",
  "harness 優化", "工作流建議", or "ASCII 圖", or wants to understand how a Claude Code
  session behaved and what to improve next. Uses classify_session_pattern as the hard-signal
  source and turns it into actionable workflow guidance.
---

# CTA Usage Pattern — Session Pattern Analysis

Analyze one or more sessions with the MCP classifier and convert the result into concrete harness guidance.

## Output Language

Configured output language: `${user_config.output_language}`.

- `en`: use English for human-readable prose, headings, and table labels.
- `zh-TW`: use Traditional Chinese for human-readable prose, headings, and table labels.
- `auto`, unset, empty, unsupported values, or a literal unexpanded placeholder: follow the latest user message's primary natural language; if that is unclear, fall back to English.
- Keep technical identifiers such as metric names, tool names, pattern IDs, JSON fields, and session IDs in English.

## Workflow

### Step 1: Freshness
Execute `mcp__token-analyzer__sync_db` when the user asks for "latest" or when the conversation likely depends on newly-created sessions.
For a direct historical `session_id` lookup, `sync_db` is optional because `classify_session_pattern` reads JSONL directly.

### Step 2: Pick Sessions
If the user provides a `session_id`, use it directly with `mcp__token-analyzer__classify_session_pattern`.

If the user does not provide a `session_id`:
1. Execute `mcp__token-analyzer__analyze_global`
2. Select up to 3 candidate sessions from `top_sessions`
3. Execute `mcp__token-analyzer__classify_session_pattern` for each selected session
4. Summarize the pattern mix, highest-severity result first

### Step 3: Map Signals to Advice
Use the local skill reference file `references/harness-signals-to-advice.md` as the SSOT mapping.

Required output elements:
- detected `pattern`
- `severity`
- short signal summary (`cache_hit_rate`, `subagent_count`, `repeated_edit_peak`, `turn_count`, `duration_minutes`, `topic_shift_count`)
- 2-4 concrete workflow adjustments

### Step 4: Optional Sparkline
If the user asks for trend context, execute `mcp__token-analyzer__trend_report` and render a short Unicode sparkline using the returned token totals.
Keep it inline, for example:
`14d token trend: ▁▂▃▅▄▆█`

## Reporting Template

Localize every human-readable label and sentence; the English example below defines structure only.

```markdown
## CTA Usage Pattern Analysis — a1b2c3d4

- Pattern: `correction_spiral`
- Severity: `alert`
- Signals: cache_hit_rate 18.0%, repeated_edit_peak 8, output_token_ratio 61.0%, turn_count 42

### Recommendations
1. Split large files into smaller edit units to avoid repeatedly patching the same file.
2. Request diff-only responses to reduce output token growth.
3. If requirements changed, start a new session or checkpoint before continuing edits.
```

## Rules

1. Quote exact numeric signals from MCP output; do not invent percentages or counts.
2. When severity is `info`, keep the tone observational instead of warning-heavy.
3. When classifying multiple sessions, order by severity first, then by cost if available.
4. If the MCP tool returns `AMBIGUOUS_SESSION_ID`, ask the user for a longer ID rather than guessing.
