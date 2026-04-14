# CTA Tool Reference

Quick reference for the 8 token-analyzer MCP tools. For full architecture details, see the source README at `${CLAUDE_PLUGIN_ROOT}/README.md`.

## sync_db

| Field | Value |
|-------|-------|
| Parameters | None |
| Returns | `files_synced`, `sessions_upserted`, `files_failed` |
| When | Latest/freshness-sensitive workflows，或需要先把 JSONL 匯入 SQLite 的分析流程 |
| Note | Incremental (mtime-based), idempotent. `classify_session_pattern` 直接讀 JSONL，歷史 session lookup 不強制依賴 `sync_db` |

## classify_session_pattern

| Field | Value |
|-------|-------|
| Parameters | `session_id` (required, full UUID or unique prefix) |
| Returns | `pattern`, `signals`, `severity`, `evidence` |
| When | Session usage-pattern diagnosis and workflow advice |
| Errors | Symbolic code in `error.data.code`: `SESSION_NOT_FOUND`, `AMBIGUOUS_SESSION_ID`, `PARSE_FAILED`, `INSUFFICIENT_DATA` |
| Note | Reads the matching JSONL directly, so it works even before `sync_db` catches up |

## analyze_session

| Field | Value |
|-------|-------|
| Parameters | `session_id` (required) |
| Returns | 10-dimension analysis: token breakdown (input/output/cache_creation/cache_read + percentages), cache_hit_rate, CostBreakdown, model_breakdown, tool_ranking, compression_events, total_turns, avg_tokens_per_turn, is_subagent, duration_range |
| When | Deep dive into a single session |

## analyze_project

| Field | Value |
|-------|-------|
| Parameters | `project_path` (required), `include_subagents` (default: true), `sort_by`: cost\|tokens\|date (default: cost), `limit` (default: 10) |
| Returns | ProjectStats, top_sessions, tool_ranking (with session_count), model_distribution, subagent_ratio |
| When | Project-level aggregation |
| Tip | Set `include_subagents=false` to calculate subagent overhead |

## analyze_global

| Field | Value |
|-------|-------|
| Parameters | None |
| Returns | GlobalStats (total_sessions, total_projects, total_tokens, total_cost_usd, avg_cache_hit_rate), project_ranking, top_sessions (20), tool_ranking, subagent_ratio |
| When | Cross-project panoramic view |

## cost_report

| Field | Value |
|-------|-------|
| Parameters | `month`: YYYY-MM (default: current), `daily` (default: true), `project_path` (optional) |
| Returns | Monthly total_cost, daily_breakdown (date, token types, cost, session_count), project_breakdown, model_breakdown |
| When | Monthly cost reporting |

## anomaly_scan

| Field | Value |
|-------|-------|
| Parameters | `stddev_threshold` (default: 2.0), `project_path` (optional), `min_tokens_for_cache_check` (default: 10000) |
| Returns | anomalies list (session_id, anomaly_type, description, value, threshold, stddevs_above, severity), sessions_scanned |
| When | Statistical anomaly detection |

### stddev_threshold Tuning

| Value | Sensitivity | Use case |
|-------|------------|----------|
| 1.5 | High | Catch borderline anomalies |
| 2.0 | Balanced | Default, clear outliers |
| 3.0 | Conservative | Only extreme anomalies |

### min_tokens_for_cache_check Tuning

| Value | Sensitivity | Use case |
|-------|------------|----------|
| 5000 | Aggressive | Include medium sessions |
| 10000 | Balanced | Default |
| 50000 | Conservative | Only long sessions |

### 6 Anomaly Types

1. **CostInefficient** — High cost + low cache (composite, most dangerous). Severity = cost x (1 - cache_hit_rate)
2. **HighCost** — Cost exceeds mean + N stddev. Has severity scoring.
3. **LowCacheHitRate** — Cache rate below mean - N stddev (only sessions >= min_tokens). Has severity scoring.
4. **ExcessiveToolUse** — Tool call count exceeds mean + N stddev (possible infinite loop).
5. **HighTokenUsage** — Total tokens exceed mean + N stddev.
6. **UnusualModelMix** — Session uses >= 3 different models.

### Known Caveat

anomaly_scan output can exceed **370K characters**. Always write to `${TMPDIR:-/tmp}/cta-anomaly-report.json` before parsing.

## trend_report

| Field | Value |
|-------|-------|
| Parameters | `granularity`: daily\|weekly\|monthly (default: daily), `last_n_days` (default: 30), `project_path` (optional) |
| Returns | data_points (DailyStats array), total_days, avg_daily_cost, avg_daily_tokens, peak_day |
| When | Time-series trend analysis |
