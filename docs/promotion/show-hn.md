# Show HN Post

## Timing

**Post at:** Sunday 0-2 UTC (Taiwan 8-10am)
**Rationale:** 15.7% breakout rate vs weekday 9.5%

## Title

```
Show HN: I scanned 7,593 Claude Code sessions – here's where the tokens actually go
```

## Body

```
I analyzed 7,593 Claude Code sessions (~8,363 million tokens, $7,132 total cost)
and found patterns I never expected:

- 8.0% of sessions had statistical anomalies
- LowCacheHitRate was the #1 issue (239 instances) — prompts
  that could have been cached were re-sent every time
- 59 sessions were "cost-inefficient" — high cost + poor cache
- ExcessiveToolUse flagged 297 sessions making far more tool calls
  than typical

Built a Claude Code plugin that diagnoses 6 types of token waste with
severity scoring. Fully local — parses your ~/.claude JSONL files into
SQLite. Nothing leaves your machine. No cloud, no telemetry.

Install: claude plugin install claude-token-analyzer
Then ask: "cta" or "how much did I spend?"

MIT licensed: https://github.com/li195111/claude-token-analyzer
```

## Data Source

Data from CTA scan on 2026-03-30:
- Total sessions: 7,593
- Total tokens: 8,362,998,382 (~8,363M)
- Total cost: $7,132.64
- Unique sessions with anomalies: 609 (8.0%)
- LowCacheHitRate: 239 instances
- ExcessiveToolUse: 297 instances
- CostInefficient: 59 instances
- HighTokenUsage: 166 instances
- HighCost: 159 instances

## Post-Launch Checklist

- [ ] Monitor HN for 1-2 hours after posting
- [ ] Respond to privacy questions with "fully local, SQLite, nothing leaves your machine"
- [ ] If asked about ccusage comparison: "ccusage shows how much you spent; CTA diagnoses where you're wasting and why"
- [ ] Upvote does NOT violate HN rules (asking others to upvote does)

## Narrative Formula Check

- [x] Fear/curiosity hook: "found patterns I never expected"
- [x] Large-scale real data: 7,593 sessions, 8,363 million tokens
- [x] Specific findings: LowCacheHitRate #1, CostInefficient, ExcessiveToolUse
- [x] One-line install: `claude plugin install claude-token-analyzer`
- [x] Fully local trust signal: "Nothing leaves your machine. No cloud, no telemetry."
