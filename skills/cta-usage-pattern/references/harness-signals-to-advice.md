# Harness Signals To Advice

This file is the SSOT mapping from `classify_session_pattern` output to workflow guidance.

## cold_session
- Focus on cache warmth: keep the same session alive when the task is still coherent.
- Avoid changing foundational context files mid-session unless necessary.
- Prefer one stable model per working block when cache behavior matters.

## correction_spiral
- Ask for diff-only responses before another large rewrite.
- Split the target file into smaller edit scopes or checkpoint and restart with a narrower ask.
- Re-state the acceptance target before the next edit if requirements drifted.

## subagent_swarm
- Only fan out subagents for independent work streams.
- Keep ownership boundaries explicit so results can be merged without rework.
- If coordination cost dominates, collapse back to one main agent.

## kitchen_sink
- Checkpoint and start a new session when the task meaningfully changes.
- Avoid stacking unrelated "順便" asks into the same long context.
- Keep one session focused on one deliverable or one debugging thread.

## marathon
- This is usually healthy deep-work behavior; keep the session stable.
- Save checkpoints before large pivots so the long cache-warm run stays coherent.
- Watch for a late-session shift into correction_spiral or kitchen_sink patterns.

## observer
- Good reconnaissance pattern: read, grep, and scope before editing.
- When the goal becomes implementation, open a fresh focused session or clearly switch modes.

## normal
- No clear anti-pattern detected.
- Continue the current workflow unless another metric or user constraint suggests otherwise.
