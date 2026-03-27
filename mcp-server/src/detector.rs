use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::storage::Database;
use crate::types::{CompressionEvent, ParseResult};

// === Types ===

/// Types of anomalies that can be detected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnomalyType {
    HighTokenUsage,
    HighCost,
    UnusualModelMix,
    ExcessiveToolUse,
    LowCacheHitRate,
    CostInefficient,
}

/// A single anomaly detected in a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    pub session_id: String,
    pub anomaly_type: AnomalyType,
    pub description: String,
    pub value: f64,
    pub threshold: f64,
    pub stddevs_above: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<f64>,
}

/// Report from anomaly detection scan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyReport {
    pub anomalies: Vec<Anomaly>,
    pub sessions_scanned: u64,
    pub stddev_threshold: f64,
    pub min_tokens_for_cache_check: u64,
}

/// Higher-level compression analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionAnalysis {
    pub events: Vec<CompressionEvent>,
    pub total_compressions: usize,
    pub estimated_tokens_recovered: u64,
    pub has_compact_agent: bool,
}

// === Detection functions ===

/// Scan for anomalous sessions using statistical analysis
/// stddev_threshold: how many standard deviations above mean to flag (default 2.0)
pub fn detect_anomalies(
    db: &Database,
    stddev_threshold: f64,
    project_path: Option<&str>,
    min_tokens_for_cache_check: u64,
) -> Result<AnomalyReport> {
    let sessions = db.all_sessions(project_path)?;
    let sessions_scanned = sessions.len() as u64;

    if sessions.is_empty() {
        return Ok(AnomalyReport {
            anomalies: Vec::new(),
            sessions_scanned: 0,
            stddev_threshold,
            min_tokens_for_cache_check,
        });
    }

    let mut anomalies = Vec::new();

    // Collect per-session metrics
    let total_tokens_per_session: Vec<u64> = sessions
        .iter()
        .map(|s| {
            s.total_input_tokens
                + s.total_output_tokens
                + s.total_cache_creation_tokens
                + s.total_cache_read_tokens
        })
        .collect();

    let token_values: Vec<f64> = total_tokens_per_session.iter().map(|&t| t as f64).collect();

    let cost_values: Vec<f64> = sessions
        .iter()
        .map(|s| s.total_cost_usd.unwrap_or(0.0))
        .collect();

    let turns_values: Vec<f64> = sessions.iter().map(|s| s.total_turns as f64).collect();

    // Only collect cache_hit_values for sessions above the min token threshold
    let cache_hit_values: Vec<f64> = sessions
        .iter()
        .zip(total_tokens_per_session.iter())
        .filter_map(|(s, &total_tokens)| {
            if total_tokens >= min_tokens_for_cache_check {
                s.cache_hit_rate
            } else {
                None
            }
        })
        .collect();

    // HighTokenUsage
    let (mean_tokens, stddev_tokens) = mean_stddev(&token_values);
    let high_token_threshold = mean_tokens + stddev_tokens * stddev_threshold;
    for (i, val) in token_values.iter().enumerate() {
        if stddev_tokens > 0.0 && *val > high_token_threshold {
            let stddevs = (*val - mean_tokens) / stddev_tokens;
            anomalies.push(Anomaly {
                session_id: sessions[i].session_id.clone(),
                anomaly_type: AnomalyType::HighTokenUsage,
                description: format!(
                    "Total tokens ({}) is {:.1} stddevs above mean ({:.0})",
                    *val as u64, stddevs, mean_tokens
                ),
                value: *val,
                threshold: high_token_threshold,
                stddevs_above: stddevs,
                severity: None,
            });
        }
    }

    // HighCost — with severity scoring
    let (mean_cost, stddev_cost) = mean_stddev(&cost_values);
    let high_cost_threshold = mean_cost + stddev_cost * stddev_threshold;
    for (i, val) in cost_values.iter().enumerate() {
        if stddev_cost > 0.0 && *val > high_cost_threshold {
            let stddevs = (*val - mean_cost) / stddev_cost;
            let cache_rate = sessions[i].cache_hit_rate.unwrap_or(0.0);
            let sev = *val * (1.0 - cache_rate);
            anomalies.push(Anomaly {
                session_id: sessions[i].session_id.clone(),
                anomaly_type: AnomalyType::HighCost,
                description: format!(
                    "Cost (${:.4}) is {:.1} stddevs above mean (${:.4})",
                    val, stddevs, mean_cost
                ),
                value: *val,
                threshold: high_cost_threshold,
                stddevs_above: stddevs,
                severity: Some(sev),
            });
        }
    }

    // ExcessiveToolUse (using turns as proxy)
    let (mean_turns, stddev_turns) = mean_stddev(&turns_values);
    let high_turns_threshold = mean_turns + stddev_turns * stddev_threshold;
    for (i, val) in turns_values.iter().enumerate() {
        if stddev_turns > 0.0 && *val > high_turns_threshold {
            let stddevs = (*val - mean_turns) / stddev_turns;
            anomalies.push(Anomaly {
                session_id: sessions[i].session_id.clone(),
                anomaly_type: AnomalyType::ExcessiveToolUse,
                description: format!(
                    "Turn count ({}) is {:.1} stddevs above mean ({:.0})",
                    *val as u64, stddevs, mean_turns
                ),
                value: *val,
                threshold: high_turns_threshold,
                stddevs_above: stddevs,
                severity: None,
            });
        }
    }

    // LowCacheHitRate (sessions BELOW mean - stddev * threshold)
    // Only considers sessions with total_tokens >= min_tokens_for_cache_check
    if !cache_hit_values.is_empty() {
        let (mean_cache, stddev_cache) = mean_stddev(&cache_hit_values);
        let low_cache_threshold = mean_cache - stddev_cache * stddev_threshold;
        if stddev_cache > 0.0 {
            for (idx, session) in sessions.iter().enumerate() {
                let total_tokens = total_tokens_per_session[idx];
                if total_tokens < min_tokens_for_cache_check {
                    continue;
                }
                if let Some(chr) = session.cache_hit_rate
                    && chr < low_cache_threshold
                {
                    let stddevs = (mean_cache - chr) / stddev_cache;
                    let cost = session.total_cost_usd.unwrap_or(0.0);
                    let sev = cost * (1.0 - chr);
                    anomalies.push(Anomaly {
                        session_id: session.session_id.clone(),
                        anomaly_type: AnomalyType::LowCacheHitRate,
                        description: format!(
                            "Cache hit rate ({:.2}%) is {:.1} stddevs below mean ({:.2}%)",
                            chr * 100.0,
                            stddevs,
                            mean_cache * 100.0
                        ),
                        value: chr,
                        threshold: low_cache_threshold,
                        stddevs_above: stddevs,
                        severity: Some(sev),
                    });
                }
            }
        }

        // CostInefficient: composite anomaly (cost > mean AND cache < mean AND tokens >= threshold)
        // Uses looser thresholds (mean, not mean+Nσ) because the cross-dimensional combination is the signal
        for (idx, session) in sessions.iter().enumerate() {
            let total_tokens = total_tokens_per_session[idx];
            if total_tokens < min_tokens_for_cache_check {
                continue;
            }
            let cost = session.total_cost_usd.unwrap_or(0.0);
            let cache_rate = match session.cache_hit_rate {
                Some(c) => c,
                None => continue,
            };
            if cost > mean_cost && cache_rate < mean_cache {
                let sev = cost * (1.0 - cache_rate);
                anomalies.push(Anomaly {
                    session_id: session.session_id.clone(),
                    anomaly_type: AnomalyType::CostInefficient,
                    description: format!(
                        "High cost (${:.4}, mean ${:.4}) with low cache hit rate ({:.2}%, mean {:.2}%)",
                        cost, mean_cost, cache_rate * 100.0, mean_cache * 100.0
                    ),
                    value: sev,
                    threshold: mean_cost, // used mean_cost as the cost threshold
                    stddevs_above: 0.0,   // not applicable for composite check
                    severity: Some(sev),
                });
            }
        }
    }

    // UnusualModelMix: sessions using 3+ distinct models
    let multi_model_sessions = db.sessions_with_model_count(3)?;
    for (session_id, model_count) in &multi_model_sessions {
        // Optionally filter by project if specified
        if let Some(pp) = project_path {
            let is_in_project = sessions
                .iter()
                .any(|s| s.session_id == *session_id && s.project_path == pp);
            if !is_in_project {
                continue;
            }
        }
        anomalies.push(Anomaly {
            session_id: session_id.clone(),
            anomaly_type: AnomalyType::UnusualModelMix,
            description: format!(
                "Session uses {} distinct models (threshold: 3)",
                model_count
            ),
            value: *model_count as f64,
            threshold: 3.0,
            stddevs_above: 0.0, // not applicable for this check
            severity: None,
        });
    }

    Ok(AnomalyReport {
        anomalies,
        sessions_scanned,
        stddev_threshold,
        min_tokens_for_cache_check,
    })
}

/// Analyze compression events from a parsed session
pub fn analyze_compression(result: &ParseResult) -> CompressionAnalysis {
    let events = result.compression_events.clone();
    let total_compressions = events.len();

    let estimated_tokens_recovered: u64 = events.iter().map(|e| e.cache_read_before).sum();

    let has_compact_agent = result
        .agent_id
        .as_ref()
        .is_some_and(|id| id.contains("compact"));

    CompressionAnalysis {
        events,
        total_compressions,
        estimated_tokens_recovered,
        has_compact_agent,
    }
}

/// Calculate mean and standard deviation for a set of values
fn mean_stddev(values: &[f64]) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0);
    }

    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;

    if values.len() < 2 {
        return (mean, 0.0);
    }

    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    let stddev = variance.sqrt();

    (mean, stddev)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pricing::PricingTable;
    use crate::types::{CompressionEvent, ModelUsage, ParseResult, ToolUsageStat};
    use chrono::{TimeZone, Utc};

    fn test_pricing() -> PricingTable {
        PricingTable::embedded()
    }

    fn make_db_parse_result(
        session_id: &str,
        project_path: &str,
        input: u64,
        output: u64,
        cache_creation: u64,
        cache_read: u64,
        timestamp_str: &str,
        model: &str,
        turns: u64,
    ) -> ParseResult {
        let ts = timestamp_str.parse::<chrono::DateTime<Utc>>().ok();

        ParseResult {
            session_id: session_id.to_string(),
            project_path: project_path.to_string(),
            is_subagent: false,
            agent_id: None,
            first_timestamp: ts,
            last_timestamp: ts,
            assistant_turns: vec![],
            model_usage: vec![ModelUsage {
                model: model.to_string(),
                turn_count: turns,
                total_input: input,
                total_output: output,
                total_cache_creation: cache_creation,
                total_cache_read: cache_read,
            }],
            tool_usage: vec![ToolUsageStat {
                name: "Read".to_string(),
                invocation_count: turns,
            }],
            compression_events: vec![],
            total_turns: turns,
            total_input_tokens: input,
            total_output_tokens: output,
            total_cache_creation_tokens: cache_creation,
            total_cache_read_tokens: cache_read,
            failed_lines: 0,
            total_lines: 10,
            turn_durations_ms: vec![],
        }
    }

    fn make_multi_model_parse_result(
        session_id: &str,
        project_path: &str,
        timestamp_str: &str,
        models: &[&str],
    ) -> ParseResult {
        let ts = timestamp_str.parse::<chrono::DateTime<Utc>>().ok();

        let model_usage: Vec<ModelUsage> = models
            .iter()
            .map(|m| ModelUsage {
                model: m.to_string(),
                turn_count: 1,
                total_input: 100,
                total_output: 50,
                total_cache_creation: 0,
                total_cache_read: 0,
            })
            .collect();

        let total_input: u64 = model_usage.iter().map(|m| m.total_input).sum();
        let total_output: u64 = model_usage.iter().map(|m| m.total_output).sum();

        ParseResult {
            session_id: session_id.to_string(),
            project_path: project_path.to_string(),
            is_subagent: false,
            agent_id: None,
            first_timestamp: ts,
            last_timestamp: ts,
            assistant_turns: vec![],
            model_usage,
            tool_usage: vec![],
            compression_events: vec![],
            total_turns: models.len() as u64,
            total_input_tokens: total_input,
            total_output_tokens: total_output,
            total_cache_creation_tokens: 0,
            total_cache_read_tokens: 0,
            failed_lines: 0,
            total_lines: 10,
            turn_durations_ms: vec![],
        }
    }

    #[test]
    fn test_detect_anomalies_high_cost() {
        let db = Database::open_in_memory().unwrap();
        let pricing = test_pricing();

        // Insert several normal sessions
        for i in 0..10 {
            let r = make_db_parse_result(
                &format!("sess-normal-{}", i),
                "/project/a",
                1000,
                500,
                0,
                0,
                "2026-03-20T10:00:00Z",
                "claude-sonnet-4-20250514",
                5,
            );
            db.upsert_session(&r, &pricing).unwrap();
        }

        // Insert one outlier with 100x more tokens
        let outlier = make_db_parse_result(
            "sess-outlier",
            "/project/a",
            100_000,
            50_000,
            10_000,
            80_000,
            "2026-03-20T11:00:00Z",
            "claude-sonnet-4-20250514",
            50,
        );
        db.upsert_session(&outlier, &pricing).unwrap();

        let report = detect_anomalies(&db, 2.0, None, 0).unwrap();

        assert_eq!(report.sessions_scanned, 11);
        assert!(
            !report.anomalies.is_empty(),
            "Should detect at least one anomaly"
        );

        // Should have at least HighTokenUsage and HighCost for the outlier
        let high_cost = report
            .anomalies
            .iter()
            .filter(|a| {
                matches!(a.anomaly_type, AnomalyType::HighCost) && a.session_id == "sess-outlier"
            })
            .count();
        assert!(high_cost > 0, "Should detect HighCost anomaly for outlier");

        let high_tokens = report
            .anomalies
            .iter()
            .filter(|a| {
                matches!(a.anomaly_type, AnomalyType::HighTokenUsage)
                    && a.session_id == "sess-outlier"
            })
            .count();
        assert!(
            high_tokens > 0,
            "Should detect HighTokenUsage anomaly for outlier"
        );
    }

    #[test]
    fn test_detect_anomalies_no_anomalies() {
        let db = Database::open_in_memory().unwrap();
        let pricing = test_pricing();

        // Insert sessions with identical metrics
        for i in 0..5 {
            let r = make_db_parse_result(
                &format!("sess-same-{}", i),
                "/project/a",
                1000,
                500,
                200,
                800,
                "2026-03-20T10:00:00Z",
                "claude-sonnet-4-20250514",
                5,
            );
            db.upsert_session(&r, &pricing).unwrap();
        }

        let report = detect_anomalies(&db, 2.0, None, 0).unwrap();

        assert_eq!(report.sessions_scanned, 5);
        // All sessions identical => stddev = 0 => no statistical anomalies
        // (UnusualModelMix might still fire if they use 3+ models, but they don't here)
        let statistical_anomalies: Vec<_> = report
            .anomalies
            .iter()
            .filter(|a| !matches!(a.anomaly_type, AnomalyType::UnusualModelMix))
            .collect();
        assert!(
            statistical_anomalies.is_empty(),
            "Should have no statistical anomalies when all sessions are identical: {:?}",
            statistical_anomalies
        );
    }

    #[test]
    fn test_detect_anomalies_low_cache_hit() {
        let db = Database::open_in_memory().unwrap();
        let pricing = test_pricing();

        // Insert sessions with good cache hit rates
        for i in 0..10 {
            let r = make_db_parse_result(
                &format!("sess-good-cache-{}", i),
                "/project/a",
                1000,
                500,
                200,
                5000, // high cache read => good hit rate
                "2026-03-20T10:00:00Z",
                "claude-sonnet-4-20250514",
                5,
            );
            db.upsert_session(&r, &pricing).unwrap();
        }

        // Insert one session with very low cache hit rate
        let low_cache = make_db_parse_result(
            "sess-low-cache",
            "/project/a",
            5000,
            500,
            3000,
            10, // very low cache read => poor hit rate
            "2026-03-20T11:00:00Z",
            "claude-sonnet-4-20250514",
            5,
        );
        db.upsert_session(&low_cache, &pricing).unwrap();

        let report = detect_anomalies(&db, 2.0, None, 0).unwrap();

        let low_cache_anomalies: Vec<_> = report
            .anomalies
            .iter()
            .filter(|a| {
                matches!(a.anomaly_type, AnomalyType::LowCacheHitRate)
                    && a.session_id == "sess-low-cache"
            })
            .collect();

        assert!(
            !low_cache_anomalies.is_empty(),
            "Should detect LowCacheHitRate anomaly. All anomalies: {:?}",
            report.anomalies
        );
    }

    #[test]
    fn test_detect_anomalies_unusual_model_mix() {
        let db = Database::open_in_memory().unwrap();
        let pricing = test_pricing();

        // Insert normal single-model sessions
        for i in 0..5 {
            let r = make_db_parse_result(
                &format!("sess-single-model-{}", i),
                "/project/a",
                1000,
                500,
                0,
                0,
                "2026-03-20T10:00:00Z",
                "claude-sonnet-4-20250514",
                5,
            );
            db.upsert_session(&r, &pricing).unwrap();
        }

        // Insert a session using 3 different models
        let multi = make_multi_model_parse_result(
            "sess-multi-model",
            "/project/a",
            "2026-03-20T11:00:00Z",
            &[
                "claude-sonnet-4-20250514",
                "claude-opus-4-6",
                "claude-haiku-3-5-20241022",
            ],
        );
        db.upsert_session(&multi, &pricing).unwrap();

        let report = detect_anomalies(&db, 2.0, None, 0).unwrap();

        let model_mix_anomalies: Vec<_> = report
            .anomalies
            .iter()
            .filter(|a| {
                matches!(a.anomaly_type, AnomalyType::UnusualModelMix)
                    && a.session_id == "sess-multi-model"
            })
            .collect();

        assert!(
            !model_mix_anomalies.is_empty(),
            "Should detect UnusualModelMix anomaly for session with 3 models"
        );
    }

    #[test]
    fn test_analyze_compression() {
        let ts = Utc.with_ymd_and_hms(2026, 3, 20, 10, 0, 0).unwrap();

        let result = ParseResult {
            session_id: "sess-comp".to_string(),
            project_path: "/project/c".to_string(),
            is_subagent: false,
            agent_id: None,
            first_timestamp: Some(ts),
            last_timestamp: Some(ts),
            assistant_turns: vec![],
            model_usage: vec![],
            tool_usage: vec![],
            compression_events: vec![
                CompressionEvent {
                    turn_index: 5,
                    timestamp: ts,
                    cache_read_before: 10000,
                    cache_read_after: 500,
                    drop_percentage: 0.95,
                },
                CompressionEvent {
                    turn_index: 15,
                    timestamp: ts,
                    cache_read_before: 8000,
                    cache_read_after: 400,
                    drop_percentage: 0.95,
                },
            ],
            total_turns: 20,
            total_input_tokens: 5000,
            total_output_tokens: 2000,
            total_cache_creation_tokens: 1000,
            total_cache_read_tokens: 3000,
            failed_lines: 0,
            total_lines: 40,
            turn_durations_ms: vec![],
        };

        let analysis = analyze_compression(&result);

        assert_eq!(analysis.total_compressions, 2);
        assert_eq!(analysis.estimated_tokens_recovered, 18000); // 10000 + 8000
        assert!(!analysis.has_compact_agent);
        assert_eq!(analysis.events.len(), 2);
    }

    #[test]
    fn test_analyze_compression_compact_agent() {
        let ts = Utc.with_ymd_and_hms(2026, 3, 20, 10, 0, 0).unwrap();

        let result = ParseResult {
            session_id: "sess-compact".to_string(),
            project_path: "/project/c".to_string(),
            is_subagent: true,
            agent_id: Some("agent-compact-42a1c7".to_string()),
            first_timestamp: Some(ts),
            last_timestamp: Some(ts),
            assistant_turns: vec![],
            model_usage: vec![],
            tool_usage: vec![],
            compression_events: vec![CompressionEvent {
                turn_index: 3,
                timestamp: ts,
                cache_read_before: 5000,
                cache_read_after: 200,
                drop_percentage: 0.96,
            }],
            total_turns: 5,
            total_input_tokens: 1000,
            total_output_tokens: 500,
            total_cache_creation_tokens: 0,
            total_cache_read_tokens: 200,
            failed_lines: 0,
            total_lines: 10,
            turn_durations_ms: vec![],
        };

        let analysis = analyze_compression(&result);

        assert!(analysis.has_compact_agent);
        assert_eq!(analysis.total_compressions, 1);
        assert_eq!(analysis.estimated_tokens_recovered, 5000);
    }

    #[test]
    fn test_detect_anomalies_empty_db() {
        let db = Database::open_in_memory().unwrap();

        let report = detect_anomalies(&db, 2.0, None, 0).unwrap();

        assert_eq!(report.sessions_scanned, 0);
        assert!(report.anomalies.is_empty());
    }

    #[test]
    fn test_mean_stddev() {
        // Simple case
        let values = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let (mean, stddev) = mean_stddev(&values);
        assert!((mean - 5.0).abs() < 0.01);
        assert!((stddev - 2.0).abs() < 0.01);

        // Empty
        let (mean, stddev) = mean_stddev(&[]);
        assert_eq!(mean, 0.0);
        assert_eq!(stddev, 0.0);

        // Single element
        let (mean, stddev) = mean_stddev(&[42.0]);
        assert_eq!(mean, 42.0);
        assert_eq!(stddev, 0.0);
    }

    #[test]
    fn test_detect_anomalies_project_filter() {
        let db = Database::open_in_memory().unwrap();
        let pricing = test_pricing();

        // Insert sessions in project A
        for i in 0..5 {
            let r = make_db_parse_result(
                &format!("sess-a-{}", i),
                "/project/a",
                1000,
                500,
                0,
                0,
                "2026-03-20T10:00:00Z",
                "claude-sonnet-4-20250514",
                5,
            );
            db.upsert_session(&r, &pricing).unwrap();
        }

        // Insert sessions in project B
        for i in 0..5 {
            let r = make_db_parse_result(
                &format!("sess-b-{}", i),
                "/project/b",
                1000,
                500,
                0,
                0,
                "2026-03-20T10:00:00Z",
                "claude-sonnet-4-20250514",
                5,
            );
            db.upsert_session(&r, &pricing).unwrap();
        }

        // Filter to project A only
        let report = detect_anomalies(&db, 2.0, Some("/project/a"), 0).unwrap();
        assert_eq!(report.sessions_scanned, 5);

        // No anomalies since all sessions are identical
        let statistical = report
            .anomalies
            .iter()
            .filter(|a| !matches!(a.anomaly_type, AnomalyType::UnusualModelMix))
            .count();
        assert_eq!(statistical, 0);
    }

    #[test]
    fn test_low_cache_min_token_filter() {
        let db = Database::open_in_memory().unwrap();
        let pricing = test_pricing();

        // Insert sessions with good cache hit rates and enough tokens
        for i in 0..10 {
            let r = make_db_parse_result(
                &format!("sess-good-{}", i),
                "/project/a",
                5000,
                2000,
                1000,
                20000, // high cache read => good hit rate, total ~28000
                "2026-03-20T10:00:00Z",
                "claude-sonnet-4-20250514",
                10,
            );
            db.upsert_session(&r, &pricing).unwrap();
        }

        // Insert a SHORT session with low cache hit rate (total tokens = 110, way below 10000)
        let short_low_cache = make_db_parse_result(
            "sess-short-low-cache",
            "/project/a",
            50,
            50,
            0,
            10, // low cache read => poor hit rate, but total only 110
            "2026-03-20T11:00:00Z",
            "claude-sonnet-4-20250514",
            1,
        );
        db.upsert_session(&short_low_cache, &pricing).unwrap();

        // With min_tokens = 10000, the short session should be filtered out
        let report = detect_anomalies(&db, 2.0, None, 10_000).unwrap();
        let low_cache = report
            .anomalies
            .iter()
            .filter(|a| {
                matches!(a.anomaly_type, AnomalyType::LowCacheHitRate)
                    && a.session_id == "sess-short-low-cache"
            })
            .count();
        assert_eq!(
            low_cache, 0,
            "Short session should NOT trigger LowCacheHitRate with min_tokens filter"
        );

        // With min_tokens = 0, it should be detected (old behavior)
        let report_no_filter = detect_anomalies(&db, 2.0, None, 0).unwrap();
        let low_cache_unfiltered = report_no_filter
            .anomalies
            .iter()
            .filter(|a| {
                matches!(a.anomaly_type, AnomalyType::LowCacheHitRate)
                    && a.session_id == "sess-short-low-cache"
            })
            .count();
        assert!(
            low_cache_unfiltered > 0,
            "Short session SHOULD trigger LowCacheHitRate without min_tokens filter"
        );
    }

    #[test]
    fn test_cost_inefficient_detection() {
        let db = Database::open_in_memory().unwrap();
        let pricing = test_pricing();

        // Insert cheap sessions with good cache (baseline)
        for i in 0..8 {
            let r = make_db_parse_result(
                &format!("sess-cheap-{}", i),
                "/project/a",
                1000,
                500,
                200,
                10000, // good cache, total ~11700
                "2026-03-20T10:00:00Z",
                "claude-sonnet-4-20250514",
                5,
            );
            db.upsert_session(&r, &pricing).unwrap();
        }

        // Insert EXPENSIVE session with LOW cache (should trigger CostInefficient)
        let expensive_low_cache = make_db_parse_result(
            "sess-expensive-low-cache",
            "/project/a",
            50000,
            20000,
            5000,
            100, // low cache read, total ~75100
            "2026-03-20T11:00:00Z",
            "claude-sonnet-4-20250514",
            30,
        );
        db.upsert_session(&expensive_low_cache, &pricing).unwrap();

        // Insert EXPENSIVE session with HIGH cache (should NOT trigger CostInefficient)
        // cache_hit_rate = 500000 / (50000 + 5000 + 500000) ≈ 0.90, well above mean
        let expensive_high_cache = make_db_parse_result(
            "sess-expensive-high-cache",
            "/project/a",
            50000,
            20000,
            5000,
            500000, // very high cache read => cache rate ~0.90, total ~575000
            "2026-03-20T12:00:00Z",
            "claude-sonnet-4-20250514",
            30,
        );
        db.upsert_session(&expensive_high_cache, &pricing).unwrap();

        let report = detect_anomalies(&db, 2.0, None, 10_000).unwrap();

        let cost_inefficient: Vec<_> = report
            .anomalies
            .iter()
            .filter(|a| matches!(a.anomaly_type, AnomalyType::CostInefficient))
            .collect();

        // The expensive+low-cache session should be flagged
        let flagged_low = cost_inefficient
            .iter()
            .any(|a| a.session_id == "sess-expensive-low-cache");
        assert!(
            flagged_low,
            "Expensive session with low cache should trigger CostInefficient. Anomalies: {:?}",
            report.anomalies
        );

        // The expensive+high-cache session should NOT be flagged (cache rate above mean)
        let flagged_high = cost_inefficient
            .iter()
            .any(|a| a.session_id == "sess-expensive-high-cache");
        assert!(
            !flagged_high,
            "Expensive session with high cache should NOT trigger CostInefficient"
        );
    }

    #[test]
    fn test_severity_scoring() {
        let db = Database::open_in_memory().unwrap();
        let pricing = test_pricing();

        // Insert normal sessions
        for i in 0..5 {
            let r = make_db_parse_result(
                &format!("sess-normal-{}", i),
                "/project/a",
                1000,
                500,
                200,
                5000, // total ~6700
                "2026-03-20T10:00:00Z",
                "claude-sonnet-4-20250514",
                5,
            );
            db.upsert_session(&r, &pricing).unwrap();
        }

        // Insert outlier (high cost, low cache)
        let outlier = make_db_parse_result(
            "sess-outlier",
            "/project/a",
            100_000,
            50_000,
            10_000,
            100, // very low cache read, total ~160100
            "2026-03-20T11:00:00Z",
            "claude-sonnet-4-20250514",
            50,
        );
        db.upsert_session(&outlier, &pricing).unwrap();

        let report = detect_anomalies(&db, 2.0, None, 0).unwrap();

        // HighCost anomaly should have severity
        let high_cost: Vec<_> = report
            .anomalies
            .iter()
            .filter(|a| {
                matches!(a.anomaly_type, AnomalyType::HighCost) && a.session_id == "sess-outlier"
            })
            .collect();
        assert!(!high_cost.is_empty(), "Should have HighCost anomaly");
        assert!(
            high_cost[0].severity.is_some(),
            "HighCost should have severity score"
        );
        assert!(
            high_cost[0].severity.unwrap() > 0.0,
            "Severity should be positive"
        );

        // HighTokenUsage should NOT have severity
        let high_tokens: Vec<_> = report
            .anomalies
            .iter()
            .filter(|a| {
                matches!(a.anomaly_type, AnomalyType::HighTokenUsage)
                    && a.session_id == "sess-outlier"
            })
            .collect();
        assert!(!high_tokens.is_empty(), "Should have HighTokenUsage anomaly");
        assert!(
            high_tokens[0].severity.is_none(),
            "HighTokenUsage should NOT have severity"
        );

        // Severity formula: cost * (1.0 - cache_hit_rate)
        // Verify it's roughly cost * ~1.0 (since cache is near 0)
        let sev = high_cost[0].severity.unwrap();
        let cost_val = high_cost[0].value;
        assert!(
            sev <= cost_val * 1.01, // severity <= cost (since (1 - cache) <= 1)
            "Severity ({}) should be <= cost ({})",
            sev,
            cost_val
        );
        assert!(
            sev > cost_val * 0.5,
            "Severity ({}) should be > half of cost ({}) when cache is very low",
            sev,
            cost_val
        );
    }
}
