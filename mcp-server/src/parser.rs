use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::types::{
    AssistantTurn, CompressionEvent, ModelUsage, ParseResult, TokenUsage, ToolUsageStat,
    ToolUseInfo,
};

/// Parse a single JSONL session file into a structured ParseResult.
///
/// Handles deduplication of assistant records by requestId (only the final
/// record with a non-null stop_reason is kept). Skips non-essential record
/// types (progress, file-history-snapshot, queue-operation, last-prompt).
pub fn parse_jsonl_file(path: &Path) -> Result<ParseResult> {
    let content = fs::read_to_string(path)?;

    // Extract metadata from file path
    let (session_id, project_path, is_subagent, agent_id) = extract_path_metadata(path);

    let mut assistant_map: HashMap<String, AssistantTurn> = HashMap::new();
    let mut turn_durations_ms: Vec<u64> = Vec::new();
    let mut failed_lines: u64 = 0;
    let mut total_lines: u64 = 0;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        total_lines += 1;

        let value: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => {
                failed_lines += 1;
                continue;
            }
        };

        let record_type = value.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match record_type {
            "assistant" => {
                match parse_assistant_record(&value) {
                    Some(turn) => {
                        let key = turn.request_id.clone();
                        // Dedup: if this record has a stop_reason (final), always overwrite.
                        // If no stop_reason (partial), only insert if no entry exists yet.
                        if turn.stop_reason.is_some() {
                            assistant_map.insert(key, turn);
                        } else {
                            assistant_map.entry(key).or_insert(turn);
                        }
                    }
                    None => {
                        failed_lines += 1;
                    }
                }
            }
            "system" => {
                if let Some(dur) = value.get("turn_duration_ms").and_then(|v| v.as_u64()) {
                    turn_durations_ms.push(dur);
                }
            }
            "user" => {
                // Skip user records for now (skill detection is future work)
            }
            "progress" | "file-history-snapshot" | "queue-operation" | "last-prompt" => {
                // Skip non-essential record types
            }
            _ => {
                // Unknown type — skip silently for forward compatibility
            }
        }
    }

    // Check failure threshold: only fail if BOTH conditions are true:
    // - failed_lines > 5% of total_lines (percentage threshold)
    // - failed_lines >= 3 (absolute minimum — don't fail on 1-2 bad lines in small files)
    if total_lines > 0 {
        let failure_rate = failed_lines as f64 / total_lines as f64;
        if failure_rate > 0.05 && failed_lines >= 3 {
            bail!(
                "Too many failed lines: {failed_lines}/{total_lines} ({:.1}% > 5% threshold)",
                failure_rate * 100.0
            );
        }
    }

    // Convert HashMap to sorted Vec
    let mut assistant_turns: Vec<AssistantTurn> = assistant_map.into_values().collect();
    assistant_turns.sort_by_key(|t| t.timestamp);

    // Aggregate model usage
    let model_usage = aggregate_model_usage(&assistant_turns);

    // Aggregate tool usage
    let tool_usage = aggregate_tool_usage(&assistant_turns);

    // Detect compression events
    let compression_events = detect_compression_events(&assistant_turns);

    // Compute totals
    let total_turns = assistant_turns.len() as u64;
    let total_input_tokens: u64 = assistant_turns.iter().map(|t| t.usage.input_tokens).sum();
    let total_output_tokens: u64 = assistant_turns.iter().map(|t| t.usage.output_tokens).sum();
    let total_cache_creation_tokens: u64 = assistant_turns
        .iter()
        .map(|t| t.usage.cache_creation_input_tokens)
        .sum();
    let total_cache_read_tokens: u64 = assistant_turns
        .iter()
        .map(|t| t.usage.cache_read_input_tokens)
        .sum();

    // Timestamps
    let first_timestamp = assistant_turns.first().map(|t| t.timestamp);
    let last_timestamp = assistant_turns.last().map(|t| t.timestamp);

    Ok(ParseResult {
        session_id,
        project_path,
        is_subagent,
        agent_id,
        first_timestamp,
        last_timestamp,
        assistant_turns,
        model_usage,
        tool_usage,
        compression_events,
        total_turns,
        total_input_tokens,
        total_output_tokens,
        total_cache_creation_tokens,
        total_cache_read_tokens,
        failed_lines,
        total_lines,
        turn_durations_ms,
    })
}

/// Parse a single assistant record from a JSON Value into an AssistantTurn.
/// Returns None if essential fields (timestamp, message) are missing.
/// `requestId` is optional — API error records often lack it; a generated
/// fallback ID is used in that case.
fn parse_assistant_record(value: &Value) -> Option<AssistantTurn> {
    let request_id = value
        .get("requestId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            // Generate a deterministic fallback from uuid or timestamp
            let uuid = value
                .get("uuid")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            format!("_no_request_id_{}", uuid)
        });
    let timestamp_str = value.get("timestamp")?.as_str()?;
    let timestamp: DateTime<Utc> = timestamp_str.parse().ok()?;
    let is_sidechain = value
        .get("isSidechain")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let message = value.get("message")?;
    let model = message
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let stop_reason = message
        .get("stop_reason")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Parse usage
    let usage = parse_usage(message.get("usage"));

    // Parse tools from content array
    let tools = parse_tools(message.get("content"));

    Some(AssistantTurn {
        request_id,
        model,
        usage,
        tools,
        stop_reason,
        timestamp,
        is_sidechain,
    })
}

/// Parse token usage from the usage JSON object.
fn parse_usage(usage_value: Option<&Value>) -> TokenUsage {
    let Some(u) = usage_value else {
        return TokenUsage::default();
    };

    TokenUsage {
        input_tokens: u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
        output_tokens: u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
        cache_creation_input_tokens: u
            .get("cache_creation_input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        cache_read_input_tokens: u
            .get("cache_read_input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
    }
}

/// Parse tool_use entries from the content array.
fn parse_tools(content_value: Option<&Value>) -> Vec<ToolUseInfo> {
    let Some(content) = content_value else {
        return Vec::new();
    };
    let Some(arr) = content.as_array() else {
        return Vec::new();
    };

    arr.iter()
        .filter_map(|item| {
            let item_type = item.get("type")?.as_str()?;
            if item_type != "tool_use" {
                return None;
            }
            let name = item.get("name")?.as_str()?.to_string();
            let tool_use_id = item.get("id")?.as_str()?.to_string();
            Some(ToolUseInfo { name, tool_use_id })
        })
        .collect()
}

/// Extract metadata from the JSONL file path.
///
/// Expected paths:
/// - Main session: `.../<project-hash>/<session-uuid>.jsonl`
/// - Subagent: `.../<project-hash>/<session-uuid>/subagents/agent-<id>.jsonl`
fn extract_path_metadata(path: &Path) -> (String, String, bool, Option<String>) {
    let path_str = path.to_string_lossy();
    let is_subagent = path_str.contains("/subagents/");

    // Extract session_id from filename
    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    let (session_id, agent_id) = if is_subagent {
        // For subagent files like agent-abc123.jsonl, the agent_id is the stem
        // and the session_id comes from the grandparent directory
        let agent_id_str = file_stem.clone();

        // Walk up: subagents/ -> <session-uuid>/ -> <project-hash>/
        let session_id = path
            .parent() // subagents/
            .and_then(|p| p.parent()) // <session-uuid>/
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        (session_id, Some(agent_id_str))
    } else {
        (file_stem, None)
    };

    // Extract project_path from parent directory
    let project_path = if is_subagent {
        // grandparent of subagents dir -> session dir -> project dir
        path.parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .and_then(|p| p.to_str())
            .unwrap_or("unknown")
            .to_string()
    } else {
        path.parent()
            .and_then(|p| p.to_str())
            .unwrap_or("unknown")
            .to_string()
    };

    (session_id, project_path, is_subagent, agent_id)
}

/// Aggregate token usage per model across all turns.
fn aggregate_model_usage(turns: &[AssistantTurn]) -> Vec<ModelUsage> {
    let mut map: HashMap<String, ModelUsage> = HashMap::new();

    for turn in turns {
        let entry = map.entry(turn.model.clone()).or_insert_with(|| ModelUsage {
            model: turn.model.clone(),
            turn_count: 0,
            total_input: 0,
            total_output: 0,
            total_cache_creation: 0,
            total_cache_read: 0,
        });
        entry.turn_count += 1;
        entry.total_input += turn.usage.input_tokens;
        entry.total_output += turn.usage.output_tokens;
        entry.total_cache_creation += turn.usage.cache_creation_input_tokens;
        entry.total_cache_read += turn.usage.cache_read_input_tokens;
    }

    let mut result: Vec<ModelUsage> = map.into_values().collect();
    result.sort_by(|a, b| b.turn_count.cmp(&a.turn_count)); // most-used first
    result
}

/// Aggregate tool invocation counts across all turns.
fn aggregate_tool_usage(turns: &[AssistantTurn]) -> Vec<ToolUsageStat> {
    let mut map: HashMap<String, u64> = HashMap::new();

    for turn in turns {
        for tool in &turn.tools {
            *map.entry(tool.name.clone()).or_insert(0) += 1;
        }
    }

    let mut result: Vec<ToolUsageStat> = map
        .into_iter()
        .map(|(name, invocation_count)| ToolUsageStat {
            name,
            invocation_count,
        })
        .collect();
    result.sort_by(|a, b| b.invocation_count.cmp(&a.invocation_count)); // most-used first
    result
}

/// Detect context compression events by looking for >80% drops in cache_read
/// between consecutive assistant turns.
fn detect_compression_events(turns: &[AssistantTurn]) -> Vec<CompressionEvent> {
    let mut events = Vec::new();

    for i in 1..turns.len() {
        // Skip if models differ — model switches naturally reset cache namespace
        if turns[i - 1].model != turns[i].model {
            continue;
        }

        let before = turns[i - 1].usage.cache_read_input_tokens;
        let after = turns[i].usage.cache_read_input_tokens;

        // Only detect drops (before must be significant to avoid false positives)
        if before > 0 && after < before {
            let drop = (before - after) as f64 / before as f64;
            if drop > 0.80 {
                events.push(CompressionEvent {
                    turn_index: i,
                    timestamp: turns[i].timestamp,
                    cache_read_before: before,
                    cache_read_after: after,
                    drop_percentage: drop,
                });
            }
        }
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Helper: create a temp JSONL file with given lines and return the path
    fn write_temp_jsonl(lines: &[&str]) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");
        for line in lines {
            writeln!(file, "{}", line).expect("Failed to write line");
        }
        file
    }

    #[allow(clippy::too_many_arguments)]
    fn make_assistant_line(
        request_id: &str,
        stop_reason: Option<&str>,
        input_tokens: u64,
        output_tokens: u64,
        cache_creation: u64,
        cache_read: u64,
        model: &str,
        timestamp: &str,
        tools: &[(&str, &str)],
    ) -> String {
        let stop_reason_json = match stop_reason {
            Some(s) => format!("\"{}\"", s),
            None => "null".to_string(),
        };

        let tools_json: Vec<String> = tools
            .iter()
            .map(|(name, id)| {
                format!(
                    r#"{{"type":"tool_use","id":"{}","name":"{}","input":{{}}}}"#,
                    id, name
                )
            })
            .collect();
        let tools_str = tools_json.join(",");

        let content = if tools.is_empty() {
            r#"[{"type":"text","text":"hello"}]"#.to_string()
        } else {
            format!(r#"[{{"type":"text","text":"hello"}},{}]"#, tools_str)
        };

        format!(
            r#"{{"type":"assistant","requestId":"{}","timestamp":"{}","isSidechain":false,"message":{{"model":"{}","id":"msg_test","role":"assistant","content":{},"stop_reason":{},"usage":{{"input_tokens":{},"output_tokens":{},"cache_creation_input_tokens":{},"cache_read_input_tokens":{}}}}}}}"#,
            request_id,
            timestamp,
            model,
            content,
            stop_reason_json,
            input_tokens,
            output_tokens,
            cache_creation,
            cache_read
        )
    }

    fn make_user_line(timestamp: &str) -> String {
        format!(
            r#"{{"type":"user","timestamp":"{}","message":{{"role":"user","content":"hello"}}}}"#,
            timestamp
        )
    }

    fn make_system_line(timestamp: &str, duration_ms: Option<u64>) -> String {
        let dur = match duration_ms {
            Some(d) => format!(r#","turn_duration_ms":{}"#, d),
            None => String::new(),
        };
        format!(
            r#"{{"type":"system","timestamp":"{}","message":{{"role":"system","content":"init"}}{}}}"#,
            timestamp, dur
        )
    }

    #[test]
    fn test_parse_simple_session() {
        // 2 assistant pairs (partial + final) + 1 user record = 5 lines
        let line0 = make_assistant_line(
            "req_001", None, 0, 100, 0, 0,
            "claude-opus-4-6", "2026-03-20T06:00:00.000Z", &[],
        );
        let line1 = make_assistant_line(
            "req_001", Some("end_turn"), 500, 200, 1000, 2000,
            "claude-opus-4-6", "2026-03-20T06:00:01.000Z", &[("Read", "toolu_01")],
        );
        let line2 = make_user_line("2026-03-20T06:00:02.000Z");
        let line3 = make_assistant_line(
            "req_002", None, 0, 50, 0, 0,
            "claude-opus-4-6", "2026-03-20T06:00:03.000Z", &[],
        );
        let line4 = make_assistant_line(
            "req_002", Some("tool_use"), 600, 300, 500, 3000,
            "claude-opus-4-6", "2026-03-20T06:00:04.000Z",
            &[("Bash", "toolu_02"), ("Read", "toolu_03")],
        );
        let file = write_temp_jsonl(&[&line0, &line1, &line2, &line3, &line4]);

        let result = parse_jsonl_file(file.path()).expect("Parse should succeed");

        assert_eq!(result.total_lines, 5);
        assert_eq!(result.failed_lines, 0);
        assert_eq!(result.total_turns, 2, "Should have 2 deduplicated turns");
        assert_eq!(result.total_input_tokens, 1100); // 500 + 600
        assert_eq!(result.total_output_tokens, 500); // 200 + 300
        assert_eq!(result.total_cache_creation_tokens, 1500); // 1000 + 500
        assert_eq!(result.total_cache_read_tokens, 5000); // 2000 + 3000
        assert!(!result.is_subagent);
        assert!(result.agent_id.is_none());

        // Tool usage
        let read_stat = result.tool_usage.iter().find(|t| t.name == "Read");
        assert!(read_stat.is_some());
        assert_eq!(read_stat.unwrap().invocation_count, 2); // one in each turn

        let bash_stat = result.tool_usage.iter().find(|t| t.name == "Bash");
        assert!(bash_stat.is_some());
        assert_eq!(bash_stat.unwrap().invocation_count, 1);

        // Model usage
        assert_eq!(result.model_usage.len(), 1);
        assert_eq!(result.model_usage[0].model, "claude-opus-4-6");
        assert_eq!(result.model_usage[0].turn_count, 2);
    }

    #[test]
    fn test_parse_dedup_request_id() {
        // Two assistant records with same requestId — only final one should be counted
        let partial = make_assistant_line(
            "req_dup",
            None,
            0,
            50,
            0,
            0,
            "claude-opus-4-6",
            "2026-03-20T06:00:00.000Z",
            &[],
        );
        let final_rec = make_assistant_line(
            "req_dup",
            Some("end_turn"),
            1000,
            500,
            2000,
            3000,
            "claude-opus-4-6",
            "2026-03-20T06:00:01.000Z",
            &[("Read", "toolu_01")],
        );

        let file = write_temp_jsonl(&[&partial, &final_rec]);
        let result = parse_jsonl_file(file.path()).expect("Parse should succeed");

        assert_eq!(result.total_turns, 1, "Dedup should yield exactly 1 turn");
        assert_eq!(
            result.total_input_tokens, 1000,
            "Should use final record's tokens"
        );
        assert_eq!(result.total_output_tokens, 500);
        assert_eq!(
            result.assistant_turns[0].stop_reason,
            Some("end_turn".to_string())
        );
    }

    #[test]
    fn test_parse_empty_file() {
        let file = write_temp_jsonl(&[]);
        let result = parse_jsonl_file(file.path()).expect("Empty file should parse OK");

        assert_eq!(result.total_lines, 0);
        assert_eq!(result.total_turns, 0);
        assert_eq!(result.total_input_tokens, 0);
        assert_eq!(result.total_output_tokens, 0);
        assert!(result.assistant_turns.is_empty());
        assert!(result.model_usage.is_empty());
        assert!(result.tool_usage.is_empty());
        assert!(result.first_timestamp.is_none());
        assert!(result.last_timestamp.is_none());
    }

    #[test]
    fn test_parse_failure_threshold() {
        // 10 lines: 9 garbage + 1 valid → 90% failure rate → should Err
        let valid = make_assistant_line(
            "req_ok",
            Some("end_turn"),
            100,
            50,
            0,
            0,
            "claude-opus-4-6",
            "2026-03-20T06:00:00.000Z",
            &[],
        );
        let mut lines: Vec<String> = (0..9).map(|i| format!("not valid json {}", i)).collect();
        lines.push(valid);
        let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();

        let file = write_temp_jsonl(&refs);
        let result = parse_jsonl_file(file.path());

        assert!(result.is_err(), "Should fail when >5% lines are bad");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Too many failed lines"),
            "Error message should mention failed lines: {}",
            err_msg
        );
    }

    #[test]
    fn test_parse_failure_threshold_small_file_tolerant() {
        // Small file: 4 lines, 1 garbage line → 25% failure rate but only 1 failed line
        // With the new absolute minimum threshold (failed_lines >= 3), this should succeed
        let valid1 = make_assistant_line(
            "req_1", Some("end_turn"), 100, 50, 0, 0,
            "claude-opus-4-6", "2026-03-20T06:00:00.000Z", &[],
        );
        let valid2 = make_assistant_line(
            "req_2", Some("end_turn"), 200, 100, 0, 0,
            "claude-opus-4-6", "2026-03-20T06:00:01.000Z", &[],
        );
        let user = make_user_line("2026-03-20T06:00:02.000Z");
        let garbage = "not valid json";

        let file = write_temp_jsonl(&[&valid1, &valid2, &user, garbage]);
        let result = parse_jsonl_file(file.path());

        assert!(
            result.is_ok(),
            "Small file with 1 bad line should not fail (absolute minimum = 3): {:?}",
            result.err()
        );
        let parsed = result.unwrap();
        assert_eq!(parsed.failed_lines, 1);
        assert_eq!(parsed.total_lines, 4);
        assert_eq!(parsed.total_turns, 2);
    }

    #[test]
    fn test_parse_assistant_missing_request_id() {
        // An assistant record without requestId (e.g. API error) should still parse
        // using a fallback ID, not count as a failure
        let line = r#"{"type":"assistant","uuid":"uuid-test-123","timestamp":"2026-03-20T06:00:00.000Z","isSidechain":false,"isApiErrorMessage":true,"message":{"model":"claude-opus-4-6","id":"msg_test","role":"assistant","content":[{"type":"text","text":"error"}],"stop_reason":"stop_sequence","usage":{"input_tokens":100,"output_tokens":50,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}}"#.to_string();
        let user = make_user_line("2026-03-20T06:00:01.000Z");

        let file = write_temp_jsonl(&[&line, &user]);
        let result = parse_jsonl_file(file.path()).expect("Should parse OK with missing requestId");

        assert_eq!(result.total_turns, 1, "Should have 1 assistant turn");
        assert_eq!(result.failed_lines, 0, "Missing requestId should not count as failure");
        assert!(
            result.assistant_turns[0].request_id.starts_with("_no_request_id_"),
            "Should use fallback request_id, got: {}",
            result.assistant_turns[0].request_id
        );
    }

    #[test]
    fn test_subagent_detection() {
        // Create a temp file in a path structure that looks like subagent
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let subagent_dir = dir
            .path()
            .join("project-hash")
            .join("session-uuid")
            .join("subagents");
        std::fs::create_dir_all(&subagent_dir).expect("Failed to create dirs");

        let agent_file = subagent_dir.join("agent-compact-42a1c7.jsonl");
        let line = make_assistant_line(
            "req_sub",
            Some("end_turn"),
            100,
            50,
            0,
            0,
            "claude-opus-4-6",
            "2026-03-20T06:00:00.000Z",
            &[],
        );
        std::fs::write(&agent_file, format!("{}\n", line)).expect("Failed to write");

        let result = parse_jsonl_file(&agent_file).expect("Parse should succeed");

        assert!(result.is_subagent, "Should detect subagent from path");
        assert_eq!(
            result.agent_id,
            Some("agent-compact-42a1c7".to_string()),
            "Should extract agent_id from filename"
        );
        assert_eq!(result.session_id, "session-uuid");
    }

    #[test]
    fn test_compression_detection() {
        // Turn 0: cache_read = 10000
        // Turn 1: cache_read = 1000 (90% drop → compression event)
        // Turn 2: cache_read = 900 (10% drop → NOT compression)
        let t0 = make_assistant_line(
            "req_c0",
            Some("end_turn"),
            100,
            50,
            0,
            10000,
            "claude-opus-4-6",
            "2026-03-20T06:00:00.000Z",
            &[],
        );
        let t1 = make_assistant_line(
            "req_c1",
            Some("end_turn"),
            100,
            50,
            0,
            1000,
            "claude-opus-4-6",
            "2026-03-20T06:00:01.000Z",
            &[],
        );
        let t2 = make_assistant_line(
            "req_c2",
            Some("end_turn"),
            100,
            50,
            0,
            900,
            "claude-opus-4-6",
            "2026-03-20T06:00:02.000Z",
            &[],
        );

        let file = write_temp_jsonl(&[&t0, &t1, &t2]);
        let result = parse_jsonl_file(file.path()).expect("Parse should succeed");

        assert_eq!(
            result.compression_events.len(),
            1,
            "Should detect exactly 1 compression event"
        );
        let event = &result.compression_events[0];
        assert_eq!(event.turn_index, 1);
        assert_eq!(event.cache_read_before, 10000);
        assert_eq!(event.cache_read_after, 1000);
        assert!((event.drop_percentage - 0.9).abs() < 0.001);
    }

    #[test]
    fn test_system_turn_duration() {
        let sys = make_system_line("2026-03-20T06:00:00.000Z", Some(12345));
        let sys2 = make_system_line("2026-03-20T06:00:01.000Z", None);
        let file = write_temp_jsonl(&[&sys, &sys2]);
        let result = parse_jsonl_file(file.path()).expect("Parse should succeed");

        assert_eq!(result.turn_durations_ms, vec![12345]);
    }

    #[test]
    fn test_skipped_record_types() {
        // These should be silently skipped, not counted as failures
        let lines: [&str; 4] = [
            r#"{"type":"progress","timestamp":"2026-03-20T06:00:00.000Z"}"#,
            r#"{"type":"file-history-snapshot","timestamp":"2026-03-20T06:00:01.000Z"}"#,
            r#"{"type":"queue-operation","timestamp":"2026-03-20T06:00:02.000Z"}"#,
            r#"{"type":"last-prompt","timestamp":"2026-03-20T06:00:03.000Z"}"#,
        ];

        let file = write_temp_jsonl(&lines);
        let result = parse_jsonl_file(file.path()).expect("Parse should succeed");

        assert_eq!(result.total_lines, 4);
        assert_eq!(
            result.failed_lines, 0,
            "Skipped types should not count as failures"
        );
        assert_eq!(result.total_turns, 0);
    }

    #[test]
    fn test_sidechain_flag() {
        let line = r#"{"type":"assistant","requestId":"req_sc","timestamp":"2026-03-20T06:00:00.000Z","isSidechain":true,"message":{"model":"claude-opus-4-6","id":"msg_test","role":"assistant","content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn","usage":{"input_tokens":100,"output_tokens":50,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}}"#.to_string();
        let file = write_temp_jsonl(&[&line]);
        let result = parse_jsonl_file(file.path()).expect("Parse should succeed");

        assert_eq!(result.assistant_turns.len(), 1);
        assert!(result.assistant_turns[0].is_sidechain);
    }

    #[test]
    fn test_compression_not_triggered_by_model_switch() {
        // Turn 1: opus with high cache_read, Turn 2: sonnet with zero cache_read
        // This is a model switch, not compression.
        let line1 = make_assistant_line(
            "req_001", Some("end_turn"), 500, 200, 0, 50000,
            "claude-opus-4-6", "2026-03-20T06:00:00.000Z", &[],
        );
        let line2 = make_assistant_line(
            "req_002", Some("end_turn"), 500, 200, 0, 0,
            "claude-sonnet-4-5", "2026-03-20T06:00:01.000Z", &[],
        );

        let file = write_temp_jsonl(&[&line1, &line2]);
        let result = parse_jsonl_file(file.path()).unwrap();

        assert_eq!(result.assistant_turns.len(), 2);
        assert!(
            result.compression_events.is_empty(),
            "Model switch (opus->sonnet) should NOT trigger compression detection"
        );
    }

    #[test]
    fn test_compression_still_detected_same_model() {
        // Turn 1: opus with high cache_read, Turn 2: same opus with near-zero cache_read
        // This IS compression (same model, >80% drop).
        let line1 = make_assistant_line(
            "req_001", Some("end_turn"), 500, 200, 0, 50000,
            "claude-opus-4-6", "2026-03-20T06:00:00.000Z", &[],
        );
        let line2 = make_assistant_line(
            "req_002", Some("end_turn"), 500, 200, 0, 3000,
            "claude-opus-4-6", "2026-03-20T06:00:01.000Z", &[],
        );

        let file = write_temp_jsonl(&[&line1, &line2]);
        let result = parse_jsonl_file(file.path()).unwrap();

        assert_eq!(result.assistant_turns.len(), 2);
        assert_eq!(
            result.compression_events.len(), 1,
            "Same-model cache drop (50000->3000 = 94%) should be detected as compression"
        );
        let event = &result.compression_events[0];
        assert!(event.drop_percentage > 0.80);
        assert_eq!(event.cache_read_before, 50000);
        assert_eq!(event.cache_read_after, 3000);
    }
}
