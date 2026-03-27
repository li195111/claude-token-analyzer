use std::path::Path;

use claude_token_analyzer::analyzer::analyze_session;
use claude_token_analyzer::archiver::Archiver;
use claude_token_analyzer::detector::{analyze_compression, detect_anomalies};
use claude_token_analyzer::parser::parse_jsonl_file;
use claude_token_analyzer::pricing::PricingTable;
use claude_token_analyzer::storage::Database;

// === Fixture paths ===

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

// === E2E Tests ===

/// Test the full pipeline: parse -> analyze -> verify (simple happy path)
#[test]
fn test_simple_session_e2e() {
    let result = parse_jsonl_file(&fixture("simple-session.jsonl")).unwrap();

    // 5 lines total: 2 partial + 2 final (same requestIds) + 1 user = 2 deduplicated turns
    assert_eq!(result.total_turns, 2);
    assert!(result.total_input_tokens > 0);
    assert_eq!(result.failed_lines, 0);
    assert_eq!(result.total_lines, 5);

    let pricing = PricingTable::embedded();
    let analysis = analyze_session(&result, &pricing);

    assert!(analysis.total_cost_usd > 0.0);
    assert!(analysis.cache_hit_rate >= 0.0 && analysis.cache_hit_rate <= 1.0);
    assert!(!analysis.tool_ranking.is_empty());

    // Verify tool ranking contains Read and Bash
    let tool_names: Vec<&str> = analysis
        .tool_ranking
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert!(tool_names.contains(&"Read"), "Should have Read tool");
    assert!(tool_names.contains(&"Bash"), "Should have Bash tool");
}

/// Test multi-model session: opus + sonnet, model breakdown correctness
#[test]
fn test_multi_model_e2e() {
    let result = parse_jsonl_file(&fixture("multi-model-session.jsonl")).unwrap();

    assert!(
        result.model_usage.len() >= 2,
        "Should have at least 2 models, got {}",
        result.model_usage.len()
    );

    let pricing = PricingTable::embedded();
    let analysis = analyze_session(&result, &pricing);

    assert!(
        analysis.model_breakdown.len() >= 2,
        "Should have at least 2 model breakdowns"
    );

    // Find opus and sonnet breakdowns
    let opus = analysis
        .model_breakdown
        .iter()
        .find(|m| m.model.contains("opus"));
    let sonnet = analysis
        .model_breakdown
        .iter()
        .find(|m| m.model.contains("sonnet"));
    assert!(opus.is_some(), "Should have opus in model breakdown");
    assert!(sonnet.is_some(), "Should have sonnet in model breakdown");

    // Cost percentages should sum to ~100%
    let total_cost_pct: f64 = analysis.model_breakdown.iter().map(|m| m.cost_pct).sum();
    assert!(
        (total_cost_pct - 100.0).abs() < 0.1,
        "Cost percentages should sum to 100%, got {}",
        total_cost_pct
    );
}

/// Test compression detection via the full parse -> detect pipeline
#[test]
fn test_compression_detection_e2e() {
    let result = parse_jsonl_file(&fixture("compression-session.jsonl")).unwrap();

    assert!(
        !result.compression_events.is_empty(),
        "Should detect compression (50000 -> 3000 is a 94% drop)"
    );

    let compression = analyze_compression(&result);
    assert!(
        compression.total_compressions >= 1,
        "Should have at least 1 compression event"
    );
    assert!(
        compression.estimated_tokens_recovered > 0,
        "Should estimate recovered tokens"
    );
}

/// Test that model switches do NOT trigger false-positive compression detection.
/// Model switches naturally reset cache namespace, so cache_read dropping to 0
/// is expected behavior, not compression.
#[test]
fn test_false_positive_cache_drop_e2e() {
    let result = parse_jsonl_file(&fixture("false-positive-cache-drop.jsonl")).unwrap();

    assert_eq!(result.total_turns, 2);
    assert!(
        result.model_usage.len() >= 2,
        "Should have both opus and sonnet"
    );

    // With model-switch guard, this should NOT be flagged as compression
    assert!(
        result.compression_events.is_empty(),
        "Model switch should not trigger compression detection (was false positive, now fixed)"
    );
}

/// Test empty file parses to zero-state without errors
#[test]
fn test_empty_file_e2e() {
    let result = parse_jsonl_file(&fixture("empty.jsonl")).unwrap();

    assert_eq!(result.total_turns, 0);
    assert_eq!(result.total_lines, 0);
    assert_eq!(result.failed_lines, 0);

    let pricing = PricingTable::embedded();
    let analysis = analyze_session(&result, &pricing);

    assert_eq!(analysis.total_cost_usd, 0.0);
    assert_eq!(analysis.cache_hit_rate, 0.0);
    assert!(analysis.tool_ranking.is_empty());
    assert!(analysis.model_breakdown.is_empty());
}

/// Test that malformed.jsonl with >5% bad lines triggers an error
#[test]
fn test_malformed_exceeds_threshold() {
    let result = parse_jsonl_file(&fixture("malformed.jsonl"));

    assert!(
        result.is_err(),
        "Should fail when >5% lines are malformed (15/20 = 75%)"
    );

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Too many failed lines"),
        "Error should mention failed lines threshold: {}",
        err_msg
    );
}

/// Test file with only progress/system records (no assistant records)
#[test]
fn test_no_assistant_records_e2e() {
    let result = parse_jsonl_file(&fixture("no-assistant-records.jsonl")).unwrap();

    assert_eq!(result.total_turns, 0);
    assert!(
        result.total_lines > 0,
        "File has content but no assistant turns"
    );
    assert_eq!(
        result.failed_lines, 0,
        "Progress/system records should not be failures"
    );

    // System records with turn_duration_ms should be captured
    assert!(
        !result.turn_durations_ms.is_empty(),
        "Should capture turn_duration_ms from system records"
    );
}

/// Test forward compatibility: unknown fields should be gracefully ignored
#[test]
fn test_unknown_fields_forward_compat() {
    let result = parse_jsonl_file(&fixture("unknown-fields.jsonl")).unwrap();

    assert!(
        result.total_turns > 0,
        "Should parse successfully despite unknown fields"
    );
    assert_eq!(
        result.failed_lines, 0,
        "Unknown fields should not cause parse failures"
    );
    assert_eq!(result.total_turns, 3);
}

/// Test large session (50+ turns) for correctness and aggregation
#[test]
fn test_large_session_e2e() {
    let result = parse_jsonl_file(&fixture("large-session.jsonl")).unwrap();

    assert!(
        result.total_turns >= 50,
        "Should have at least 50 turns, got {}",
        result.total_turns
    );

    let pricing = PricingTable::embedded();
    let analysis = analyze_session(&result, &pricing);

    assert!(analysis.total_cost_usd > 0.0);
    assert!(
        analysis.tool_ranking.len() > 1,
        "Large session should use multiple tools, got {}",
        analysis.tool_ranking.len()
    );

    // Verify model breakdown has both opus and sonnet
    assert!(
        analysis.model_breakdown.len() >= 2,
        "Large session alternates models, should have >= 2"
    );
}

/// Test DB sync and query: parse -> upsert -> global_stats -> tool_ranking
#[test]
fn test_db_sync_and_query_e2e() {
    let db = Database::open_in_memory().unwrap();
    let pricing = PricingTable::embedded();

    // Parse and upsert a few fixtures
    let simple = parse_jsonl_file(&fixture("simple-session.jsonl")).unwrap();
    let multi = parse_jsonl_file(&fixture("multi-model-session.jsonl")).unwrap();
    let large = parse_jsonl_file(&fixture("large-session.jsonl")).unwrap();

    db.upsert_session(&simple, &pricing).unwrap();
    db.upsert_session(&multi, &pricing).unwrap();
    db.upsert_session(&large, &pricing).unwrap();

    // Global stats
    let stats = db.global_stats().unwrap();
    assert_eq!(stats.total_sessions, 3);
    assert!(stats.total_cost_usd > 0.0);
    assert!(stats.total_tokens > 0);

    // Tool ranking
    let tools = db.global_tool_ranking().unwrap();
    assert!(!tools.is_empty(), "Should have tool usage data");

    // Verify Read appears in the tool ranking (present in all fixtures)
    let has_read = tools.iter().any(|t| t.name == "Read");
    assert!(has_read, "Read should appear in global tool ranking");
}

/// Test anomaly detection with baseline + outlier sessions
#[test]
fn test_anomaly_detection_e2e() {
    let db = Database::open_in_memory().unwrap();
    let pricing = PricingTable::embedded();

    // Insert several normal (simple) sessions to create a baseline
    let simple = parse_jsonl_file(&fixture("simple-session.jsonl")).unwrap();
    let large = parse_jsonl_file(&fixture("large-session.jsonl")).unwrap();

    // Clone simple with different session_ids
    for i in 0..5 {
        let mut s = simple.clone();
        s.session_id = format!("simple-{}", i);
        db.upsert_session(&s, &pricing).unwrap();
    }

    // Insert the large session as an outlier
    db.upsert_session(&large, &pricing).unwrap();

    let report = detect_anomalies(&db, 1.5, None, 0).unwrap();

    assert!(
        report.sessions_scanned >= 6,
        "Should scan at least 6 sessions, got {}",
        report.sessions_scanned
    );

    // The large session should be flagged as anomalous (much higher tokens/cost)
    let has_large_anomaly = report
        .anomalies
        .iter()
        .any(|a| a.session_id == large.session_id);
    assert!(
        has_large_anomaly,
        "Large session should be detected as anomalous. Anomalies: {:?}",
        report
            .anomalies
            .iter()
            .map(|a| &a.session_id)
            .collect::<Vec<_>>()
    );
}

/// Test zstd archive round-trip: compress -> decompress -> byte-exact match
#[test]
fn test_zstd_roundtrip_e2e() {
    let tmp = tempfile::tempdir().unwrap();
    let archiver = Archiver::new(tmp.path());

    let source = fixture("simple-session.jsonl");
    let entry = archiver.archive_file(&source).unwrap();

    assert!(
        entry.compressed_size < entry.original_size,
        "zstd should compress: compressed={} original={}",
        entry.compressed_size,
        entry.original_size
    );

    // Restore and compare byte-for-byte
    let restore_path = tmp.path().join("restored.jsonl");
    archiver.restore_file(&entry, &restore_path).unwrap();

    let original = std::fs::read(&source).unwrap();
    let restored = std::fs::read(&restore_path).unwrap();
    assert_eq!(
        original,
        restored,
        "Round-trip should be lossless: original {} bytes vs restored {} bytes",
        original.len(),
        restored.len()
    );
}

/// Test subagent session: isSidechain markers are captured
#[test]
fn test_subagent_session_e2e() {
    let result = parse_jsonl_file(&fixture("subagent-session.jsonl")).unwrap();

    assert_eq!(result.total_turns, 3);
    assert_eq!(result.failed_lines, 0);

    // All turns should have is_sidechain=true
    for turn in &result.assistant_turns {
        assert!(
            turn.is_sidechain,
            "All turns in subagent-session should be sidechain, but {} is not",
            turn.request_id
        );
    }

    // Verify tools are captured
    let tool_names: Vec<&str> = result.tool_usage.iter().map(|t| t.name.as_str()).collect();
    assert!(tool_names.contains(&"Grep"), "Should have Grep tool");
    assert!(tool_names.contains(&"Read"), "Should have Read tool");
}

/// Test that compression analysis correctly identifies recovered tokens
#[test]
fn test_compression_analysis_details() {
    let result = parse_jsonl_file(&fixture("compression-session.jsonl")).unwrap();
    let compression = analyze_compression(&result);

    // Turn 0: cache_read=50000, Turn 1: cache_read=3000 (94% drop)
    // estimated_tokens_recovered sums cache_read_before for each compression event
    assert!(
        compression.estimated_tokens_recovered >= 50000,
        "Should recover at least 50000 tokens, got {}",
        compression.estimated_tokens_recovered
    );
    assert!(
        !compression.has_compact_agent,
        "Not a compact agent session"
    );
}

/// Test that multiple fixtures can coexist in DB with correct isolation
#[test]
fn test_multi_fixture_db_isolation() {
    let db = Database::open_in_memory().unwrap();
    let pricing = PricingTable::embedded();

    let simple = parse_jsonl_file(&fixture("simple-session.jsonl")).unwrap();
    let multi = parse_jsonl_file(&fixture("multi-model-session.jsonl")).unwrap();
    let unknown = parse_jsonl_file(&fixture("unknown-fields.jsonl")).unwrap();
    let large = parse_jsonl_file(&fixture("large-session.jsonl")).unwrap();

    db.upsert_session(&simple, &pricing).unwrap();
    db.upsert_session(&multi, &pricing).unwrap();
    db.upsert_session(&unknown, &pricing).unwrap();
    db.upsert_session(&large, &pricing).unwrap();

    let stats = db.global_stats().unwrap();
    assert_eq!(stats.total_sessions, 4);

    // Each session should have distinct cost
    let top = db.top_sessions_by_cost(10).unwrap();
    assert_eq!(top.len(), 4);

    // The large session should be the most expensive
    assert_eq!(
        top[0].session_id, large.session_id,
        "Large session should be the most expensive"
    );
}
