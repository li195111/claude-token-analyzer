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

```markdown
### 🔴 Session a1b2c3d4 — CostInefficient (severity: 8.5)
- **成本**：$X.XX（超出平均 X.Xσ）
- **Cache 命中率**：X.X%（平均 XX.X%）
- **模型**：claude-opus-4-6
- **Tool 排名**：Read(35), Bash(12), Edit(8)
- **壓縮事件**：2 次（turn 8, turn 15）
- **可能原因**：上下文頻繁壓縮導致 cache 失效，大量重讀檔案
- **建議**：考慮分拆任務以減少壓縮頻率
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

- Use 繁體中文 for prose, English for technical terms.
- Currency: `$X.XX USD`.
- session_id: first 8 characters.
- Severity color coding: 🔴 high (>5), 🟡 medium (2-5), 🟢 low (<2).

## Additional Resources

For MCP tool parameter details and anomaly type definitions: `../cta/references/tool-reference.md`
