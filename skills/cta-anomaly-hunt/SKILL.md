---
name: cta-anomaly-hunt
description: |
  This skill should be used when the user asks about "anomalies", "problems", "有異常嗎",
  "哪些會話有問題", "排查", "診斷", or suspects waste or efficiency issues in Claude Code
  usage. Scans for 6 types of statistical anomalies with severity scoring and automatic
  drill-down into suspicious sessions. Can also be routed from the main cta skill.
---

# CTA Anomaly Hunt — Statistical Anomaly Detection

Scan for anomalies, triage by severity, and drill into suspicious sessions for root cause diagnosis.

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

### Step 2: Scan Anomalies

Execute `mcp__token-analyzer__anomaly_scan` with:
- `stddev_threshold`: 2.0 (default), user can adjust (1.5 sensitive / 3.0 conservative)
- `min_tokens_for_cache_check`: 10000 (default), adjustable
- `project_path`: optional

**IMPORTANT**: anomaly_scan output can exceed 370K characters. Write raw output to `${TMPDIR:-/tmp}/cta-anomaly-report.json` before parsing.

### Step 3: Triage by Severity

Sort and present anomalies in this priority order:
1. **CostInefficient** — High cost + low cache (most dangerous composite anomaly)
2. **HighCost** — Pure cost outlier
3. **LowCacheHitRate** — Efficiency problem
4. **ExcessiveToolUse** — Possible infinite loop or inefficient workflow
5. **HighTokenUsage** — Token consumption outlier
6. **UnusualModelMix** — Multi-model switching

Anomalies with severity scores sort before those without.

### Step 4: Deep Dive (Top 3-5 Sessions)

For the highest-severity anomalies, execute `mcp__token-analyzer__analyze_session` with each session_id to obtain the 10-dimension analysis.

Limit automatic drill-down to **5 sessions maximum** to avoid excessive token consumption.

### Step 5: Output Diagnostic Cards

For each analyzed session, output a diagnostic card:
Localize every human-readable label and sentence; the English example below defines structure only.

```markdown
### 🔴 Session a1b2c3d4 — CostInefficient (severity: 8.5)
- **Cost**: $X.XX (X.X standard deviations above average)
- **Cache Hit Rate**: X.X% (average XX.X%)
- **Model**: claude-opus-4-6
- **Tool Ranking**: Read(35), Bash(12), Edit(8)
- **Compression Events**: 2 (turn 8, turn 15)
- **Possible Cause**: frequent context compression reduced cache reuse and caused repeated reads
- **Recommendation**: split the task to reduce compression frequency
```

Infer "possible cause" and "suggestion" from the 10-dimension analysis data:
- High compression events → context window overflow, suggest splitting tasks
- High tool use + low cache → repetitive file reading, suggest caching strategy
- Multiple models → possible failover or misconfiguration
- Excessive turns → possible stuck loop, check tool_ranking for patterns

## Behavior Rules

1. Always write raw anomaly_scan output to `${TMPDIR:-/tmp}/cta-anomaly-report.json` first.
2. Anomalies without severity (HighTokenUsage, ExcessiveToolUse, UnusualModelMix) rank after those with severity.
3. Maximum 5 automatic drill-down sessions. Offer to analyze more if the user requests.
4. User can adjust `stddev_threshold` (1.5=sensitive, 2.0=balanced, 3.0=conservative).
5. User can adjust `min_tokens_for_cache_check` to filter short sessions from cache analysis.

## Output Rules

- Currency: `$X.XX USD`.
- session_id: first 8 characters.
- Severity color coding: 🔴 high (>5), 🟡 medium (2-5), 🟢 low (<2).

## Additional Resources

For MCP tool parameter details and anomaly type definitions: `${CLAUDE_PLUGIN_ROOT}/skills/cta/references/tool-reference.md`
