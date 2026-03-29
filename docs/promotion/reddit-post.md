# Reddit r/ClaudeAI Post

## Subreddit Rules

- **Flair:** Built with Claude (REQUIRED)
- **Required elements:** What you built, How you built it, Screenshots, At least one prompt

## Timing

**Post:** 30 minutes after Show HN

## Title

```
I scanned {N} Claude Code sessions and found {ANOMALY_RATE}% had anomalies — built a plugin to diagnose them
```

## Body

```
## What I built

A Claude Code plugin that diagnoses token waste in your sessions. It detects
6 anomaly types: HighCost, LowCacheHitRate, CostInefficient, ExcessiveToolUse,
HighTokenUsage, UnusualModelMix — each with severity scoring so you know what
to fix first.

## How I built it

Rust MCP server that parses ~/.claude/projects/**/*.jsonl into a local SQLite
database, then runs statistical analysis (standard deviation thresholds +
composite anomaly detection). Exposed as 7 MCP tools + 6 workflow skills.
Nothing leaves your machine — fully local, no cloud, no telemetry.

## What I found scanning my own sessions

- {ANOMALY_RATE}% of {N} sessions had statistical anomalies
- LowCacheHitRate was #1 ({CACHE_COUNT} instances) — prompts re-sent without caching
- {COST_INEFF} sessions were "cost-inefficient" — high cost + poor cache hit rate
- ExcessiveToolUse flagged {TOOL_COUNT} sessions

## Demo

[Screenshot: anomaly scan output]
[Screenshot: cost report output]

## Try it

Install: `claude plugin install claude-token-analyzer`
Then ask: "cta" or "how much did I spend?" or "scan for anomalies"

---

也有繁體中文的 workflow skills。輸入「看看狀況」「這個月花多少」「有異常嗎」
就能得到中文分析報告。歡迎台灣/亞洲開發者試用和回饋！

GitHub: https://github.com/li195111/claude-token-analyzer
MIT Licensed.
```

## Placeholders to Fill

Same as Show HN — see show-hn.md for how to get each value.

## Pre-post Checklist

- [ ] Flair set to "Built with Claude"
- [ ] 2 screenshots prepared and uploaded (anomaly scan + cost report)
- [ ] All {PLACEHOLDER} values replaced with fresh scan data
- [ ] Chinese section included at bottom
- [ ] All 4 required elements present: What/How/Screenshots/Prompt
