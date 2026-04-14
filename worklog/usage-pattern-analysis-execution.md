# Usage Pattern Analysis Execution Log

Date: 2026-04-14
Branch: `feat/usage-pattern-analysis`
Scope: recover the prior session, clear live regressions first, finish usage-pattern implementation, and close with phase-end `linus-review-framework` checks.

## Handoff Baseline

At takeover, the branch had three real blockers:

1. `cargo test --all-targets` was red because `resolve_projects_dir()` ignored `CLAUDE_CONFIG_DIR`.
2. `pattern_classifier.rs` existed, but the public contract was incomplete:
   - `PatternResult` had no `signals`
   - `Evidence` / `PatternResult` were not serializable
   - severity/evidence logic drifted from `SPEC.md`
3. `classify_session_pattern` was not wired into `src/bin/mcp.rs`, and session lookup still lacked exact-or-unique-prefix behavior.

Secondary drift at handoff:
- docs mixed roadmap and reality
- skill docs still assumed `sync_db` was mandatory for all workflows
- worklog and registry both described already-fixed or already-existing files as missing/planned

## Review Checklist Used At Every Phase End

- over-design
- hardcode
- bad smell
- unreasonable architecture or workflow
- weak separation of concerns
- wrong design pattern
- fake assertion
- hidden red light

## Phase 1 — Path Resolution Recovery

### Implemented
- Updated `mcp-server/src/config.rs` so projects path precedence is:
  - `CTA_PROJECTS_DIR`
  - `CLAUDE_CONFIG_DIR/projects`
  - `~/.claude/projects`
- Kept DB/archive precedence unchanged.
- Added direct precedence tests for config-dir resolution, env override, and plugin-root non-interference.
- Updated path docs in `README.md`, `CLAUDE.md`, `src/bin/mcp.rs`, and feature registry references.

### Review Findings
- Hidden red light: `test_resolve_projects_dir_default` was sensitive to ambient `CLAUDE_CONFIG_DIR` in the shell and could false-fail.

### Resolution
- Explicitly clear `CLAUDE_CONFIG_DIR` inside the default-path test.
- Added direct precedence tests so the contract no longer depends on shell cleanliness.

## Phase 2 — Classifier Contract Freeze

### Implemented
- Froze MCP-8 contract in `SPEC.md` and `classify_session_pattern.feature`.
- Updated `mcp-server/src/pattern_classifier.rs`:
  - `Signals` included in every `PatternResult`
  - `Evidence` / `PatternResult` derive `Serialize` + `Deserialize`
  - stable wire casing for `Pattern`, `Severity`, `Direction`
  - spec-correct severity/evidence behavior for Cold Session, Correction Spiral, Kitchen Sink, Marathon, Observer
- Added/expanded classifier tests for alert thresholds, evidence completeness, insufficient-data messaging, and JSON contract shape.

### Review Findings
- High: removing `equal` from `Direction` made exact-threshold evidence untruthful.
- Medium: `duration_minutes` nullability still drifted between code and spec.
- Medium: insufficient-data tests were too weak and would not catch wrong error wording.

### Resolution
- Restored `Direction::Equal`.
- Updated spec to `duration_minutes: u32 | null`.
- Strengthened insufficient-data assertions to check message content, not just `is_err()`.

## Phase 3 — Signal Extraction + Session Lookup

### Implemented
- Extended `ToolUseInfo` with `file_path: Option<String>`.
- Parsed `tool_use.input.file_path` in `mcp-server/src/parser.rs`.
- Added `mcp-server/src/pattern_signals.rs` to build:
  - `cache_hit_rate`
  - `output_token_ratio`
  - `subagent_count` from `Agent`
  - `repeated_edit_peak` by file path over `Edit` / `Write` / `MultiEdit`
  - `turn_count`
  - `duration_minutes`
  - `topic_shift_count`
- Added exact-or-unique-prefix lookup in `mcp-server/src/session_finder.rs`.
- Added direct tests for parser file-path extraction, signal building, and prefix ambiguity handling.

### Review Findings
- Main risk here was hidden coupling: signal extraction could have been mixed into `bin/mcp.rs`, making it hard to test and easier to regress.

### Resolution
- Kept `pattern_signals.rs` as the thin seam between parse/analyze and classifier logic.
- Added focused tests on that seam instead of burying assertions only in MCP tests.
- Final refinement removed pricing/analyzer dependency from `classify_session_pattern`; `pattern_signals.rs` now builds directly from `ParseResult`, so a bad pricing override cannot break MCP-8 classification.

## Phase 4 — MCP-8 Delivery

### Implemented
- Added `classify_session_pattern` to `mcp-server/src/bin/mcp.rs`.
- Handler flow now is:
  - resolve exact or unique-prefix session file
  - parse JSONL
  - analyze session
  - build signals
  - classify with validation
  - serialize `PatternResult`
- Added MCP-facing error mapping for:
  - `SESSION_NOT_FOUND`
  - `AMBIGUOUS_SESSION_ID`
  - `PARSE_FAILED`
  - `INSUFFICIENT_DATA`
- Expanded MCP tests from error-only coverage to full happy-path coverage for:
  - `observer`
  - `cold_session`
  - `correction_spiral`
  - `subagent_swarm`
  - `kitchen_sink`
  - `marathon`
  - `normal`

### Review Findings
- The first happy-path test pass exposed a fake green risk: the helper constructing assistant JSONL lines was invalid and broke before real classification logic ran.

### Resolution
- Replaced brittle format-string JSON construction with `serde_json::json!` in the test helper.
- Re-ran the MCP target until all happy/error paths passed.

## Phase 5 — Delivery Surface Re-alignment

### Implemented
- Added `mcp-server/src/sparkline.rs` with direct unit tests.
- Added `skills/cta-usage-pattern/SKILL.md`.
- Added `skills/cta-usage-pattern/references/harness-signals-to-advice.md`.
- Added `tests/signal_recommendation_mapping.rs`.
- Added `.github/workflows/test.yml` for `cargo test --all-targets` and `cargo clippy --all-targets -- -D warnings`.
- Rewrote `docs/usage-pattern-analysis/FEATURE-REGISTRY.md` into a pure inventory SSOT instead of a mixed roadmap/inventory doc.
- Updated CTA skill docs so `sync_db` is freshness-sensitive, not mandatory for historical `classify_session_pattern` lookups.

### Review Findings
- Documentation drift remained the main risk:
  - registry still used stale counts and roadmap language
  - tool reference still claimed `sync_db` was always the first step
  - worklog itself still described already-fixed blockers as open

### Resolution
- Rewrote the registry into an implemented-state snapshot.
- Updated tool reference and router skill wording to match the direct-JSONL MCP-8 flow.
- Rewrote this execution log to reflect current repo truth.

## Files Added In This Delivery

- `.github/workflows/test.yml`
- `mcp-server/src/pattern_signals.rs`
- `mcp-server/src/sparkline.rs`
- `mcp-server/tests/pattern_signals.rs`
- `mcp-server/tests/signal_recommendation_mapping.rs`
- `skills/cta-usage-pattern/SKILL.md`
- `skills/cta-usage-pattern/references/harness-signals-to-advice.md`
- `docs/usage-pattern-analysis/FEATURE-REGISTRY.md` (rewritten as SSOT inventory)

## Pending Final Gate

## Codex Testing Enablement Baseline

### Discovery
- Codex already supports local MCP servers through `~/.codex/config.toml` under `[mcp_servers.*]`.
- This repo's `.mcp.json` is Claude-plugin oriented and depends on `CLAUDE_PLUGIN_ROOT` plus `scripts/run.sh`.
- MCP itself is not the blocker for Codex testing. The main blocker is skill portability:
  - `skills/cta-health-check/SKILL.md`
  - `skills/cta-cost-audit/SKILL.md`
  - `skills/cta-anomaly-hunt/SKILL.md`
  - `skills/cta-project-review/SKILL.md`
  - `skills/cta-trend-watch/SKILL.md`
  all still referenced `${CLAUDE_PLUGIN_ROOT}/skills/cta/references/tool-reference.md`.
- `skills/cta-usage-pattern/SKILL.md` was already Codex-friendly because it uses a local relative reference file.

### Implementation direction
- Remove Claude-plugin-only path assumptions from CTA skill docs by switching shared reference pointers to relative paths.
- Add a Codex-specific runner that defaults DB/archive into repo-local `.codex-test/` while keeping projects resolution overrideable.
- Add a helper script to symlink repo CTA skills into `~/.codex/skills` for native Codex skill-trigger testing.
- Add a Codex testing guide with exact config snippet, mount steps, and verification prompts.

## Codex Mount Execution

### Actions performed
- Built the release MCP binary with `bash scripts/build.sh`.
- Backed up the existing Codex config to:
  - `~/.codex/config.toml.bak.cta-codex-install-2026-04-14`
- Backed up stale pre-existing CTA skill directories under `~/.codex/skills/` to:
  - `cta.bak.cta-codex-install-2026-04-14`
  - `cta-health-check.bak.cta-codex-install-2026-04-14`
  - `cta-cost-audit.bak.cta-codex-install-2026-04-14`
  - `cta-anomaly-hunt.bak.cta-codex-install-2026-04-14`
  - `cta-project-review.bak.cta-codex-install-2026-04-14`
  - `cta-trend-watch.bak.cta-codex-install-2026-04-14`
- Installed the Codex MCP entry plus CTA skill symlinks via:
  - `bash scripts/install-codex-test-assets.sh`

### Verified system state
- `~/.codex/config.toml` now contains:
  - `[mcp_servers.token-analyzer]`
  - `command = "/Users/liyuefong/Desktop/claude-token-analyzer/scripts/run-codex.sh"`
- `~/.codex/skills/` now links the repo's current skill directories:
  - `cta`
  - `cta-health-check`
  - `cta-cost-audit`
  - `cta-anomaly-hunt`
  - `cta-project-review`
  - `cta-trend-watch`
  - `cta-usage-pattern`
- Release binary verified present at:
  - `mcp-server/target/release/cta-mcp-server`

### Operational note
- Codex must be restarted after this mount so the current session can pick up the new MCP server and skill symlinks.

## Final Verification Evidence

### `cargo test --all-targets --manifest-path mcp-server/Cargo.toml`

Raw summary:

```text
running 103 tests
test result: ok. 103 passed; 0 failed

running 12 tests
test result: ok. 12 passed; 0 failed

running 10 tests
test result: ok. 10 passed; 0 failed

running 15 tests
test result: ok. 15 passed; 0 failed

running 28 tests
test result: ok. 28 passed; 0 failed

running 2 tests
test result: ok. 2 passed; 0 failed

running 2 tests
test result: ok. 2 passed; 0 failed
```

### `cargo clippy --all-targets --manifest-path mcp-server/Cargo.toml -- -D warnings`

Raw summary:

```text
Checking claude-token-analyzer v0.1.0 (/Users/liyuefong/Desktop/claude-token-analyzer/mcp-server)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.63s
```

## Final Phase Review Record

### Cross-validation status
- QA and architect agents were re-used for final review, but two of the returned reports were stale and cited pre-fix states that no longer existed in the working tree.
- A valid reviewer finding did surface one real contract gap: MCP business error codes were documented as symbolic values, but the actual RMCP envelope only exposed generic transport codes plus a message prefix.
- Resolution applied in final pass:
  - `pattern_tool_error_to_mcp()` now emits symbolic code in `error.data.code`
  - added direct test coverage for the MCP error envelope
  - updated SPEC / feature file / tool reference to match the transport contract
- Another design smell was removed in the same final pass:
  - `classify_session_pattern` no longer loads pricing or runs `analyze_session`
  - `pattern_signals.rs` now derives all classification inputs directly from `ParseResult`
  - this prevents unrelated pricing configuration failures from breaking MCP-8
- Re-check of the remaining flagged items against current code confirmed they were false positives:
  - `PatternResult` now includes `signals` and derives serde in `mcp-server/src/pattern_classifier.rs`.
  - `ToolUseInfo.file_path` exists and is parsed in `mcp-server/src/types.rs` / `mcp-server/src/parser.rs`.
  - `config::tests::test_coexistence_plugin_root_and_config_dir_split_behavior` now passes in the final full test run.
- `codex review --uncommitted` was attempted twice during final review and both runs were interrupted by model-capacity/runtime issues before a usable findings summary was produced.

### Final blocker check
- Over-design: avoided. Large parser/storage/module splits remain outside this delivery and are not claimed as done.
- Hardcode: avoided. Thresholds remain named constants, and MCP test fixtures drive behavior without changing production thresholds.
- Separation of concerns: preserved via `pattern_signals.rs` seam instead of embedding signal extraction in `bin/mcp.rs`.
- Fake assertions: reduced. Final delta includes direct contract assertions for classifier serialization, insufficient-data messaging, session-prefix ambiguity, and MCP-8 happy paths.
- Hidden red lights: addressed. The polluted-shell config test issue and stale-doc SSOT drift were both fixed and re-verified.

### Final verdict
- No unresolved blocker remains in the current working tree after final verification.
