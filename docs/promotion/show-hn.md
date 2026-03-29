# Show HN Post

## Timing

**Post at:** Sunday 0-2 UTC (Taiwan 8-10am)
**Rationale:** 15.7% breakout rate vs weekday 9.5%

## Title

```
Show HN: I scanned {N} Claude Code sessions – here's where the tokens actually go
```

> Replace {N} with fresh scan data before posting.

## Body

```
I analyzed {N} Claude Code sessions (~{X} million tokens, ${COST} total cost)
and found patterns I never expected:

- {ANOMALY_RATE}% of sessions had statistical anomalies
- LowCacheHitRate was the #1 issue ({CACHE_COUNT} instances) — prompts
  that could have been cached were re-sent every time
- {COST_INEFF} sessions were "cost-inefficient" — high cost + poor cache
- ExcessiveToolUse flagged {TOOL_COUNT} sessions making far more tool calls
  than typical

Built a Claude Code plugin that diagnoses 6 types of token waste with
severity scoring. Fully local — parses your ~/.claude JSONL files into
SQLite. Nothing leaves your machine. No cloud, no telemetry.

Install: claude plugin install claude-token-analyzer
Then ask: "cta" or "how much did I spend?"

MIT licensed: https://github.com/li195111/claude-token-analyzer
```

## Placeholders to Fill

| Placeholder | How to Get |
|-------------|-----------|
| `{N}` | Run `cta` → health check → total sessions |
| `{X}` | Run `cta` → health check → total tokens (in millions) |
| `{COST}` | Run `cta` → cost audit → total cost |
| `{ANOMALY_RATE}` | Run `cta` → anomaly scan → anomaly rate % |
| `{CACHE_COUNT}` | Run `cta` → anomaly scan → LowCacheHitRate count |
| `{COST_INEFF}` | Run `cta` → anomaly scan → CostInefficient count |
| `{TOOL_COUNT}` | Run `cta` → anomaly scan → ExcessiveToolUse count |

## Post-Launch Checklist

- [ ] Monitor HN for 1-2 hours after posting
- [ ] Respond to privacy questions with "fully local, SQLite, nothing leaves your machine"
- [ ] If asked about ccusage comparison: "ccusage shows how much you spent; CTA diagnoses where you're wasting and why"
- [ ] Upvote does NOT violate HN rules (asking others to upvote does)

## Narrative Formula Check

- [x] Fear/curiosity hook: "found patterns I never expected"
- [x] Large-scale real data: {N} sessions, {X} million tokens
- [x] Specific findings: LowCacheHitRate #1, CostInefficient, ExcessiveToolUse
- [x] One-line install: `claude plugin install claude-token-analyzer`
- [x] Fully local trust signal: "Nothing leaves your machine. No cloud, no telemetry."
