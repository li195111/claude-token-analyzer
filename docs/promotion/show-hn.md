# Show HN Post

## Timing

**Post at:** Sunday 0-2 UTC (Taiwan 8-10am)
**Rationale:** 15.7% breakout rate vs weekday 9.5%

## Title

```
Show HN: I scanned 8,392 Claude Code sessions – here's where the tokens actually go
```

## Body

```
I analyzed 8,392 Claude Code sessions (~8.4 billion tokens, $7,132 total cost)
and found patterns I never expected:

- 1,015 anomalies detected across 5 categories
- ExcessiveToolUse was #1 (320 sessions) — far more tool calls than typical
- LowCacheHitRate hit 261 sessions — prompts that could have been cached
  were re-sent every time
- 66 sessions were "cost-inefficient" — high cost + poor cache hit rate

Built a Claude Code plugin that diagnoses 6 types of token waste with
severity scoring. Fully local — parses your ~/.claude JSONL files into
SQLite. Nothing leaves your machine. No cloud, no telemetry.

Install: claude plugin install claude-token-analyzer
Then ask: "cta" or "how much did I spend?"

MIT licensed: https://github.com/li195111/claude-token-analyzer
```

## Data Source

Data from CTA CLI scan on 2026-03-30:
- Sessions scanned: 8,392
- Total tokens: ~8.4 billion
- Total cost: $7,132.64
- Total anomaly entries: 1,015
- ExcessiveToolUse: 320 instances
- LowCacheHitRate: 261 instances
- HighTokenUsage: 194 instances
- HighCost: 174 instances
- CostInefficient: 66 instances

## Post-Launch Checklist

- [ ] Monitor HN for 1-2 hours after posting
- [ ] Respond to privacy questions with "fully local, SQLite, nothing leaves your machine"
- [ ] If asked about ccusage comparison: "ccusage shows how much you spent; CTA diagnoses where you're wasting and why"
- [ ] Upvote does NOT violate HN rules (asking others to upvote does)

## Narrative Formula Check

- [x] Fear/curiosity hook: "found patterns I never expected"
- [x] Large-scale real data: 8,392 sessions, 8.4 billion tokens
- [x] Specific findings: LowCacheHitRate #1, CostInefficient, ExcessiveToolUse
- [x] One-line install: `claude plugin install claude-token-analyzer`
- [x] Fully local trust signal: "Nothing leaves your machine. No cloud, no telemetry."
