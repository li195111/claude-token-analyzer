use std::io::Write;

use claude_token_analyzer::parser::parse_jsonl_file;
use claude_token_analyzer::pattern_signals::build_signals;
use tempfile::NamedTempFile;

fn write_temp_jsonl(lines: &[&str]) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("temp file");
    for line in lines {
        writeln!(file, "{line}").expect("write jsonl");
    }
    file
}

fn assistant_line(request_id: &str, timestamp: &str, tools_json: &str) -> String {
    format!(
        r#"{{"type":"assistant","requestId":"{request_id}","timestamp":"{timestamp}","isSidechain":false,"message":{{"model":"claude-sonnet-4-20250514","id":"msg_{request_id}","role":"assistant","content":[{{"type":"text","text":"turn"}},{tools_json}],"stop_reason":"tool_use","usage":{{"input_tokens":100,"output_tokens":50,"cache_creation_input_tokens":0,"cache_read_input_tokens":200}}}}}}"#
    )
}

#[test]
fn test_build_signals_counts_agent_edit_peak_duration_and_topic_shift() {
    let lines = [
        assistant_line(
            "req_1",
            "2026-03-20T06:00:00.000Z",
            r#"{"type":"tool_use","id":"toolu_1","name":"Read","input":{"file_path":"/src/lib.rs"}}"#,
        ),
        assistant_line(
            "req_2",
            "2026-03-20T06:01:00.000Z",
            r#"{"type":"tool_use","id":"toolu_2","name":"Grep","input":{"pattern":"todo"}}"#,
        ),
        assistant_line(
            "req_3",
            "2026-03-20T06:02:00.000Z",
            r#"{"type":"tool_use","id":"toolu_3","name":"Edit","input":{"file_path":"/src/lib.rs"}},{"type":"tool_use","id":"toolu_agent","name":"Agent","input":{"task":"review"}}"#,
        ),
        assistant_line(
            "req_4",
            "2026-03-20T06:03:00.000Z",
            r#"{"type":"tool_use","id":"toolu_4","name":"Write","input":{"file_path":"/src/lib.rs"}}"#,
        ),
        assistant_line(
            "req_5",
            "2026-03-20T06:04:00.000Z",
            r#"{"type":"tool_use","id":"toolu_5","name":"MultiEdit","input":{"file_path":"/src/lib.rs"}}"#,
        ),
    ];
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let file = write_temp_jsonl(&refs);
    let parsed = parse_jsonl_file(file.path()).expect("parse should succeed");

    let signals = build_signals(&parsed);

    assert_eq!(signals.subagent_count, 1);
    assert_eq!(signals.repeated_edit_peak, 3);
    assert_eq!(signals.turn_count, 5);
    assert_eq!(signals.duration_minutes, Some(4));
    assert_eq!(signals.topic_shift_count, 1);
    assert!((signals.output_token_ratio - (250.0 / 750.0)).abs() < 1e-9);
    assert!((signals.cache_hit_rate - (1000.0 / 1500.0)).abs() < 1e-9);
}

#[test]
fn test_build_signals_ignores_edits_without_file_paths() {
    let lines = [
        assistant_line(
            "req_1",
            "2026-03-20T06:00:00.000Z",
            r#"{"type":"tool_use","id":"toolu_1","name":"Edit","input":{}}"#,
        ),
        assistant_line(
            "req_2",
            "2026-03-20T06:01:00.000Z",
            r#"{"type":"tool_use","id":"toolu_2","name":"Write","input":{"file_path":"/src/main.rs"}}"#,
        ),
        assistant_line(
            "req_3",
            "2026-03-20T06:02:00.000Z",
            r#"{"type":"tool_use","id":"toolu_3","name":"Edit","input":{}}"#,
        ),
    ];
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let file = write_temp_jsonl(&refs);
    let parsed = parse_jsonl_file(file.path()).expect("parse should succeed");

    let signals = build_signals(&parsed);

    assert_eq!(signals.repeated_edit_peak, 1);
    assert_eq!(signals.subagent_count, 0);
    assert_eq!(signals.topic_shift_count, 0);
    assert_eq!(signals.duration_minutes, Some(2));
}
