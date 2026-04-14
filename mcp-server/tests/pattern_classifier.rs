//! Pattern classifier unit tests — TDD Phase 2 (RED)
//!
//! 這些測試參照尚未實作的 `pattern_classifier` 模組。
//! 目前應全部編譯失敗（CompileError），待 Phase 4 實作後轉為綠燈。
//!
//! 每個測試的驗收標準：
//! 1. 分類結果正確（pattern enum 值）
//! 2. severity 正確
//! 3. evidence list 包含觸發訊號

use claude_token_analyzer::pattern_classifier::{
    classify, Direction, Pattern, PatternResult, Severity, Signals,
};

// ============================================================
// Helper: 建立預設 Signals（所有值為「正常」範圍）
// ============================================================

fn normal_signals() -> Signals {
    Signals {
        cache_hit_rate: 0.65,
        output_token_ratio: 0.25,
        subagent_count: 2,
        repeated_edit_peak: 1,
        turn_count: 30,
        duration_minutes: Some(45),
        topic_shift_count: 1,
    }
}

fn has_evidence(result: &PatternResult, metric: &str) -> bool {
    result.evidence.iter().any(|e| e.metric == metric)
}

fn evidence_direction<'a>(result: &'a PatternResult, metric: &str) -> Option<&'a Direction> {
    result
        .evidence
        .iter()
        .find(|e| e.metric == metric)
        .map(|e| &e.direction)
}

fn evidence_value(result: &PatternResult, metric: &str) -> Option<f64> {
    result
        .evidence
        .iter()
        .find(|e| e.metric == metric)
        .map(|e| e.value)
}

// ============================================================
// Normal pattern
// ============================================================

#[test]
fn test_normal_session_classification() {
    let signals = normal_signals();
    let result = classify(signals);

    assert_eq!(result.pattern, Pattern::Normal);
    assert_eq!(result.severity, Severity::Info);
    assert!(
        result.evidence.is_empty(),
        "Normal session should have empty evidence"
    );
}

// ============================================================
// Cold Session
// ============================================================

#[test]
fn test_cold_session_warn() {
    let signals = Signals {
        cache_hit_rate: 0.25,
        ..normal_signals()
    };
    let result = classify(signals);

    assert_eq!(result.pattern, Pattern::ColdSession);
    assert_eq!(result.severity, Severity::Warn);
    assert!(has_evidence(&result, "cache_hit_rate"));
    assert_eq!(
        evidence_direction(&result, "cache_hit_rate"),
        Some(&Direction::Below)
    );
    assert!((evidence_value(&result, "cache_hit_rate").unwrap() - 0.25).abs() < 1e-6);
}

#[test]
fn test_cold_session_alert() {
    let signals = Signals {
        cache_hit_rate: 0.08,
        ..normal_signals()
    };
    let result = classify(signals);

    assert_eq!(result.pattern, Pattern::ColdSession);
    assert_eq!(result.severity, Severity::Alert);

    let ev = result
        .evidence
        .iter()
        .find(|e| e.metric == "cache_hit_rate")
        .unwrap();
    assert!(
        (ev.threshold - 0.15).abs() < 1e-6,
        "Alert threshold should be 0.15"
    );
}

#[test]
fn test_cold_session_boundary_exactly_at_warn_threshold() {
    // cache_hit_rate == 0.30 → NOT cold session (boundary: < 0.30 triggers)
    let signals = Signals {
        cache_hit_rate: 0.30,
        ..normal_signals()
    };
    let result = classify(signals);
    assert_ne!(
        result.pattern,
        Pattern::ColdSession,
        "0.30 is the boundary — should not trigger cold session"
    );
}

// ============================================================
// Correction Spiral
// ============================================================

#[test]
fn test_correction_spiral_warn() {
    let signals = Signals {
        repeated_edit_peak: 5,
        output_token_ratio: 0.45,
        ..normal_signals()
    };
    let result = classify(signals);

    assert_eq!(result.pattern, Pattern::CorrectionSpiral);
    assert_eq!(result.severity, Severity::Warn);
    assert!(has_evidence(&result, "repeated_edit_peak"));
    assert!(has_evidence(&result, "output_token_ratio"));
}

#[test]
fn test_correction_spiral_alert() {
    let signals = Signals {
        repeated_edit_peak: 9,
        output_token_ratio: 0.65,
        ..normal_signals()
    };
    let result = classify(signals);

    assert_eq!(result.pattern, Pattern::CorrectionSpiral);
    assert_eq!(result.severity, Severity::Alert);
}

#[test]
fn test_correction_spiral_requires_both_conditions() {
    // Only repeated_edit_peak ≥ 4, but output_token_ratio ≤ 0.40 → should NOT trigger
    let signals = Signals {
        repeated_edit_peak: 5,
        output_token_ratio: 0.35,
        ..normal_signals()
    };
    let result = classify(signals);
    assert_ne!(
        result.pattern,
        Pattern::CorrectionSpiral,
        "correction_spiral requires BOTH conditions (edit_peak ≥ 4 AND output_ratio > 0.40)"
    );
}

#[test]
fn test_correction_spiral_boundary_edit_peak() {
    // repeated_edit_peak == 3 → NOT spiral (< 4)
    let signals = Signals {
        repeated_edit_peak: 3,
        output_token_ratio: 0.50,
        ..normal_signals()
    };
    let result = classify(signals);
    assert_ne!(
        result.pattern,
        Pattern::CorrectionSpiral,
        "edit_peak of 3 should not trigger correction_spiral (threshold is 4)"
    );
}

// ============================================================
// Subagent Swarm
// ============================================================

#[test]
fn test_subagent_swarm_warn() {
    let signals = Signals {
        subagent_count: 15,
        ..normal_signals()
    };
    let result = classify(signals);

    assert_eq!(result.pattern, Pattern::SubagentSwarm);
    assert_eq!(result.severity, Severity::Warn);
    assert!(has_evidence(&result, "subagent_count"));
    assert_eq!(
        evidence_direction(&result, "subagent_count"),
        Some(&Direction::Above)
    );
}

#[test]
fn test_subagent_swarm_alert() {
    let signals = Signals {
        subagent_count: 25,
        ..normal_signals()
    };
    let result = classify(signals);

    assert_eq!(result.pattern, Pattern::SubagentSwarm);
    assert_eq!(result.severity, Severity::Alert);
}

#[test]
fn test_subagent_swarm_boundary_exactly_at_threshold() {
    // subagent_count == 10 → NOT swarm (threshold is > 10)
    let signals = Signals {
        subagent_count: 10,
        ..normal_signals()
    };
    let result = classify(signals);
    assert_ne!(
        result.pattern,
        Pattern::SubagentSwarm,
        "subagent_count == 10 should not trigger (threshold is > 10)"
    );
}

// ============================================================
// Kitchen Sink
// ============================================================

#[test]
fn test_kitchen_sink_info() {
    let signals = Signals {
        topic_shift_count: 5,
        ..normal_signals()
    };
    let result = classify(signals);

    assert_eq!(result.pattern, Pattern::KitchenSink);
    assert_eq!(result.severity, Severity::Info);
    assert!(has_evidence(&result, "topic_shift_count"));
}

#[test]
fn test_kitchen_sink_warn_at_high_shifts() {
    let signals = Signals {
        topic_shift_count: 8,
        ..normal_signals()
    };
    let result = classify(signals);

    assert_eq!(result.pattern, Pattern::KitchenSink);
    assert_eq!(result.severity, Severity::Warn);
}

// ============================================================
// Marathon
// ============================================================

#[test]
fn test_marathon_session() {
    let signals = Signals {
        turn_count: 150,
        duration_minutes: Some(185),
        cache_hit_rate: 0.82,
        ..normal_signals()
    };
    let result = classify(signals);

    assert_eq!(result.pattern, Pattern::Marathon);
    assert_eq!(result.severity, Severity::Info);
}

#[test]
fn test_marathon_requires_two_of_three_conditions() {
    // Only turns + duration (no cache_hit), should still be Marathon
    let signals = Signals {
        turn_count: 120,
        duration_minutes: Some(150),
        cache_hit_rate: 0.50, // below 0.70
        ..normal_signals()
    };
    let result = classify(signals);
    assert_eq!(
        result.pattern,
        Pattern::Marathon,
        "Marathon should trigger with 2/3 conditions met"
    );
}

// ============================================================
// Observer
// ============================================================

#[test]
fn test_observer_session() {
    let signals = Signals {
        turn_count: 12,
        repeated_edit_peak: 0,
        ..normal_signals()
    };
    let result = classify(signals);

    assert_eq!(result.pattern, Pattern::Observer);
    assert_eq!(result.severity, Severity::Info);
    assert!(has_evidence(&result, "turn_count"));
    assert_eq!(
        evidence_direction(&result, "turn_count"),
        Some(&Direction::Below)
    );
}

// ============================================================
// Priority Order Tests（優先順序：ColdSession > CorrectionSpiral > SubagentSwarm > KitchenSink > Marathon > Observer > Normal）
// ============================================================

#[test]
fn test_cold_session_takes_priority_over_correction_spiral() {
    let signals = Signals {
        cache_hit_rate: 0.12,     // triggers cold_session (alert)
        repeated_edit_peak: 5,    // triggers correction_spiral
        output_token_ratio: 0.45, // triggers correction_spiral
        ..normal_signals()
    };
    let result = classify(signals);

    assert_eq!(
        result.pattern,
        Pattern::ColdSession,
        "cold_session must take priority over correction_spiral"
    );
}

#[test]
fn test_correction_spiral_takes_priority_over_marathon() {
    let signals = Signals {
        turn_count: 150,             // marathon condition
        duration_minutes: Some(200), // marathon condition
        cache_hit_rate: 0.75,        // marathon condition
        repeated_edit_peak: 5,       // correction_spiral condition
        output_token_ratio: 0.45,    // correction_spiral condition
        ..normal_signals()
    };
    let result = classify(signals);

    assert_eq!(
        result.pattern,
        Pattern::CorrectionSpiral,
        "correction_spiral (anti-pattern) must take priority over marathon"
    );
}

#[test]
fn test_subagent_swarm_takes_priority_over_observer() {
    let signals = Signals {
        turn_count: 10,        // observer condition (< 20)
        repeated_edit_peak: 0, // observer condition (≤ 1)
        subagent_count: 15,    // subagent_swarm condition
        ..normal_signals()
    };
    let result = classify(signals);

    assert_eq!(
        result.pattern,
        Pattern::SubagentSwarm,
        "subagent_swarm must take priority over observer"
    );
}

// ============================================================
// Evidence structure correctness
// ============================================================

#[test]
fn test_evidence_contains_threshold_and_direction() {
    let signals = Signals {
        cache_hit_rate: 0.20,
        ..normal_signals()
    };
    let result = classify(signals);

    let ev = result
        .evidence
        .iter()
        .find(|e| e.metric == "cache_hit_rate")
        .expect("Should have cache_hit_rate evidence");

    // value should be the actual signal value
    assert!(
        (ev.value - 0.20).abs() < 1e-6,
        "Evidence value should match actual signal"
    );
    // threshold should be the constant (0.30 for warn)
    assert!(
        (ev.threshold - 0.30).abs() < 1e-6,
        "Evidence threshold should be 0.30 (warn boundary)"
    );
    // direction should be Below (actual < threshold)
    assert_eq!(ev.direction, Direction::Below);
}

// ============================================================
// Insufficient data guard
// ============================================================

#[test]
fn test_insufficient_turns_returns_none() {
    // Phase 4 implementation should handle this as an error or Option
    // For now we test that classify_with_validation returns Err for < 3 turns
    use claude_token_analyzer::pattern_classifier::classify_with_validation;

    let signals = Signals {
        turn_count: 2,
        ..normal_signals()
    };
    let result = classify_with_validation(signals);
    assert!(
        result.is_err(),
        "Should return Err for session with < 3 turns"
    );
}
