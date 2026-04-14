//! Session pattern classifier.
//!
//! Evaluates hard signals from a session and returns a [`PatternResult`]
//! that names the dominant usage pattern, its severity, and the evidence
//! that triggered the classification.
//!
//! # Classification pipeline
//! 1. Validate signals (minimum turn count) via [`classify_with_validation`]
//! 2. Evaluate each pattern in priority order (highest first):
//!    `ColdSession` > `CorrectionSpiral` > `SubagentSwarm` >
//!    `KitchenSink` > `Marathon` > `Observer` > `Normal`
//! 3. Return the first matching pattern with severity and evidence

use anyhow::{bail, Result};

// ============================================================
// Thresholds — all as named constants (zero hardcoding)
// ============================================================

/// Cache hit rate below this triggers `ColdSession` at `Warn` severity.
pub const COLD_SESSION_CACHE_HIT_WARN: f64 = 0.30;
/// Cache hit rate below this triggers `ColdSession` at `Alert` severity.
pub const COLD_SESSION_CACHE_HIT_ALERT: f64 = 0.15;

/// Repeated-edit peak at or above this (combined with ratio) triggers `CorrectionSpiral` Warn.
pub const CORRECTION_SPIRAL_EDIT_PEAK_WARN: u32 = 4;
/// Repeated-edit peak at or above this (combined with ratio) triggers `CorrectionSpiral` Alert.
pub const CORRECTION_SPIRAL_EDIT_PEAK_ALERT: u32 = 8;
/// Output token ratio above this (combined with edit peak) triggers `CorrectionSpiral` Warn.
pub const CORRECTION_SPIRAL_OUTPUT_RATIO_WARN: f64 = 0.40;
/// Output token ratio above this (combined with edit peak) triggers `CorrectionSpiral` Alert.
pub const CORRECTION_SPIRAL_OUTPUT_RATIO_ALERT: f64 = 0.60;

/// Subagent count above this triggers `SubagentSwarm` at `Warn` severity.
pub const SUBAGENT_SWARM_COUNT_WARN: u32 = 10;
/// Subagent count above this triggers `SubagentSwarm` at `Alert` severity.
pub const SUBAGENT_SWARM_COUNT_ALERT: u32 = 20;

/// Topic-shift count above this triggers `KitchenSink` at `Info` severity.
pub const KITCHEN_SINK_TOPIC_SHIFT_INFO: u32 = 3;
/// Topic-shift count above this triggers `KitchenSink` at `Warn` severity.
pub const KITCHEN_SINK_TOPIC_SHIFT_WARN: u32 = 6;

/// Minimum turn count for `Marathon` condition.
pub const MARATHON_TURN_COUNT: u32 = 100;
/// Minimum session duration (minutes) for `Marathon` condition.
pub const MARATHON_DURATION_MIN: u32 = 120;
/// Minimum cache hit rate for `Marathon` condition.
pub const MARATHON_CACHE_HIT: f64 = 0.70;

/// Turn count strictly below this qualifies as `Observer`.
pub const OBSERVER_MAX_TURNS: u32 = 20;
/// Repeated-edit peak at or below this qualifies as `Observer`.
pub const OBSERVER_MAX_EDIT_PEAK: u32 = 1;

/// Sessions with fewer turns than this are rejected by [`classify_with_validation`].
pub const MIN_TURNS_FOR_CLASSIFICATION: u32 = 3;

// ============================================================
// Public types
// ============================================================

/// Hard signals extracted from a single session.
#[derive(Debug, Clone)]
pub struct Signals {
    /// Fraction of input tokens served from cache (0.0–1.0).
    pub cache_hit_rate: f64,
    /// Fraction of total tokens that are output tokens.
    pub output_token_ratio: f64,
    /// Number of sub-agent invocations in the session.
    pub subagent_count: u32,
    /// Peak number of times the same file was edited within the session.
    pub repeated_edit_peak: u32,
    /// Total conversation turns.
    pub turn_count: u32,
    /// Wall-clock session duration in minutes, if available.
    pub duration_minutes: Option<u32>,
    /// Number of significant topic shifts detected in the conversation.
    pub topic_shift_count: u32,
}

/// Classified session usage pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pattern {
    /// No unusual characteristics detected.
    Normal,
    /// Cache hit rate is abnormally low — cache warmth was lost or never built.
    ColdSession,
    /// Repeated edits to the same file suggest the model is stuck in a correction loop.
    CorrectionSpiral,
    /// Unusually large number of sub-agents were spawned in a single session.
    SubagentSwarm,
    /// Multiple unrelated topics were addressed in one session (context scatter).
    KitchenSink,
    /// Long, cache-warm session — sustained deep work.
    Marathon,
    /// Short session dominated by reads/searches with minimal edits.
    Observer,
}

/// Severity level of a detected pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    /// Informational — no action required.
    Info,
    /// Warning — worth reviewing, consider adjusting workflow.
    Warn,
    /// Alert — immediate attention recommended.
    Alert,
}

/// Which side of a threshold the actual signal value fell on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Direction {
    /// Actual value is above the threshold.
    Above,
    /// Actual value is below the threshold.
    Below,
}

/// A single piece of evidence supporting the classification.
#[derive(Debug, Clone)]
pub struct Evidence {
    /// Signal name (e.g., `"cache_hit_rate"`).
    pub metric: String,
    /// Actual observed value of the signal.
    pub value: f64,
    /// The threshold that was crossed to reach this severity level.
    pub threshold: f64,
    /// Whether the actual value was above or below the threshold.
    pub direction: Direction,
}

/// Result of classifying a session's usage pattern.
#[derive(Debug, Clone)]
pub struct PatternResult {
    /// The dominant pattern detected.
    pub pattern: Pattern,
    /// Severity of the detected pattern.
    pub severity: Severity,
    /// Signals that triggered the classification (empty for `Normal`).
    pub evidence: Vec<Evidence>,
}

// ============================================================
// Public API
// ============================================================

/// Classify a session from its signals.
///
/// Patterns are evaluated in priority order; the first match wins.
/// Returns [`Pattern::Normal`] when no pattern is triggered.
///
/// Sessions with fewer than [`MIN_TURNS_FOR_CLASSIFICATION`] turns are
/// classified normally rather than erroring — use [`classify_with_validation`]
/// to enforce the minimum turn count.
pub fn classify(signals: Signals) -> PatternResult {
    if let Some(r) = check_cold_session(&signals) {
        return r;
    }
    if let Some(r) = check_correction_spiral(&signals) {
        return r;
    }
    if let Some(r) = check_subagent_swarm(&signals) {
        return r;
    }
    if let Some(r) = check_kitchen_sink(&signals) {
        return r;
    }
    if let Some(r) = check_marathon(&signals) {
        return r;
    }
    if let Some(r) = check_observer(&signals) {
        return r;
    }

    PatternResult {
        pattern: Pattern::Normal,
        severity: Severity::Info,
        evidence: vec![],
    }
}

/// Classify with input validation.
///
/// Returns `Err` if `signals.turn_count < MIN_TURNS_FOR_CLASSIFICATION`.
pub fn classify_with_validation(signals: Signals) -> Result<PatternResult> {
    if signals.turn_count < MIN_TURNS_FOR_CLASSIFICATION {
        bail!(
            "Insufficient data: session has {} turn(s), minimum is {}.",
            signals.turn_count,
            MIN_TURNS_FOR_CLASSIFICATION
        );
    }
    Ok(classify(signals))
}

// ============================================================
// Pattern checkers (private)
// ============================================================

fn check_cold_session(s: &Signals) -> Option<PatternResult> {
    if s.cache_hit_rate < COLD_SESSION_CACHE_HIT_ALERT {
        Some(PatternResult {
            pattern: Pattern::ColdSession,
            severity: Severity::Alert,
            evidence: vec![Evidence {
                metric: "cache_hit_rate".to_string(),
                value: s.cache_hit_rate,
                threshold: COLD_SESSION_CACHE_HIT_ALERT,
                direction: Direction::Below,
            }],
        })
    } else if s.cache_hit_rate < COLD_SESSION_CACHE_HIT_WARN {
        Some(PatternResult {
            pattern: Pattern::ColdSession,
            severity: Severity::Warn,
            evidence: vec![Evidence {
                metric: "cache_hit_rate".to_string(),
                value: s.cache_hit_rate,
                threshold: COLD_SESSION_CACHE_HIT_WARN,
                direction: Direction::Below,
            }],
        })
    } else {
        None
    }
}

fn check_correction_spiral(s: &Signals) -> Option<PatternResult> {
    // Both conditions must be satisfied for a match.
    let edit_alert = s.repeated_edit_peak >= CORRECTION_SPIRAL_EDIT_PEAK_ALERT;
    let ratio_alert = s.output_token_ratio > CORRECTION_SPIRAL_OUTPUT_RATIO_ALERT;
    let edit_warn = s.repeated_edit_peak >= CORRECTION_SPIRAL_EDIT_PEAK_WARN;
    let ratio_warn = s.output_token_ratio > CORRECTION_SPIRAL_OUTPUT_RATIO_WARN;

    if edit_alert && ratio_alert {
        Some(PatternResult {
            pattern: Pattern::CorrectionSpiral,
            severity: Severity::Alert,
            evidence: vec![
                Evidence {
                    metric: "repeated_edit_peak".to_string(),
                    value: s.repeated_edit_peak as f64,
                    threshold: CORRECTION_SPIRAL_EDIT_PEAK_ALERT as f64,
                    direction: Direction::Above,
                },
                Evidence {
                    metric: "output_token_ratio".to_string(),
                    value: s.output_token_ratio,
                    threshold: CORRECTION_SPIRAL_OUTPUT_RATIO_ALERT,
                    direction: Direction::Above,
                },
            ],
        })
    } else if edit_warn && ratio_warn {
        Some(PatternResult {
            pattern: Pattern::CorrectionSpiral,
            severity: Severity::Warn,
            evidence: vec![
                Evidence {
                    metric: "repeated_edit_peak".to_string(),
                    value: s.repeated_edit_peak as f64,
                    threshold: CORRECTION_SPIRAL_EDIT_PEAK_WARN as f64,
                    direction: Direction::Above,
                },
                Evidence {
                    metric: "output_token_ratio".to_string(),
                    value: s.output_token_ratio,
                    threshold: CORRECTION_SPIRAL_OUTPUT_RATIO_WARN,
                    direction: Direction::Above,
                },
            ],
        })
    } else {
        None
    }
}

fn check_subagent_swarm(s: &Signals) -> Option<PatternResult> {
    if s.subagent_count > SUBAGENT_SWARM_COUNT_ALERT {
        Some(PatternResult {
            pattern: Pattern::SubagentSwarm,
            severity: Severity::Alert,
            evidence: vec![Evidence {
                metric: "subagent_count".to_string(),
                value: s.subagent_count as f64,
                threshold: SUBAGENT_SWARM_COUNT_ALERT as f64,
                direction: Direction::Above,
            }],
        })
    } else if s.subagent_count > SUBAGENT_SWARM_COUNT_WARN {
        Some(PatternResult {
            pattern: Pattern::SubagentSwarm,
            severity: Severity::Warn,
            evidence: vec![Evidence {
                metric: "subagent_count".to_string(),
                value: s.subagent_count as f64,
                threshold: SUBAGENT_SWARM_COUNT_WARN as f64,
                direction: Direction::Above,
            }],
        })
    } else {
        None
    }
}

fn check_kitchen_sink(s: &Signals) -> Option<PatternResult> {
    if s.topic_shift_count > KITCHEN_SINK_TOPIC_SHIFT_WARN {
        Some(PatternResult {
            pattern: Pattern::KitchenSink,
            severity: Severity::Warn,
            evidence: vec![Evidence {
                metric: "topic_shift_count".to_string(),
                value: s.topic_shift_count as f64,
                threshold: KITCHEN_SINK_TOPIC_SHIFT_WARN as f64,
                direction: Direction::Above,
            }],
        })
    } else if s.topic_shift_count > KITCHEN_SINK_TOPIC_SHIFT_INFO {
        Some(PatternResult {
            pattern: Pattern::KitchenSink,
            severity: Severity::Info,
            evidence: vec![Evidence {
                metric: "topic_shift_count".to_string(),
                value: s.topic_shift_count as f64,
                threshold: KITCHEN_SINK_TOPIC_SHIFT_INFO as f64,
                direction: Direction::Above,
            }],
        })
    } else {
        None
    }
}

fn check_marathon(s: &Signals) -> Option<PatternResult> {
    let cond_turns = s.turn_count >= MARATHON_TURN_COUNT;
    let cond_duration = s
        .duration_minutes
        .map(|d| d >= MARATHON_DURATION_MIN)
        .unwrap_or(false);
    let cond_cache = s.cache_hit_rate >= MARATHON_CACHE_HIT;

    let conditions_met = [cond_turns, cond_duration, cond_cache]
        .iter()
        .filter(|&&c| c)
        .count();

    if conditions_met >= 2 {
        Some(PatternResult {
            pattern: Pattern::Marathon,
            severity: Severity::Info,
            evidence: vec![],
        })
    } else {
        None
    }
}

fn check_observer(s: &Signals) -> Option<PatternResult> {
    if s.turn_count < OBSERVER_MAX_TURNS && s.repeated_edit_peak <= OBSERVER_MAX_EDIT_PEAK {
        Some(PatternResult {
            pattern: Pattern::Observer,
            severity: Severity::Info,
            evidence: vec![Evidence {
                metric: "turn_count".to_string(),
                value: s.turn_count as f64,
                threshold: OBSERVER_MAX_TURNS as f64,
                direction: Direction::Below,
            }],
        })
    } else {
        None
    }
}
