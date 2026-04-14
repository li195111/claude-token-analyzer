use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::pricing::{CostBreakdown, PricingTable};
use crate::storage::{DailyStats, Database, GlobalStats, ProjectStats, ProjectSummary, SessionRow};
use crate::types::{CompressionEvent, ParseResult, TokenUsage, ToolUsageStat};

// === Output types ===

/// Complete analysis of a single session (from ParseResult)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionAnalysis {
    pub session_id: String,
    pub project_path: String,
    pub is_subagent: bool,
    pub duration_range: Option<(String, String)>,
    // Token breakdown (Dimension 1)
    pub total_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub input_pct: f64,
    pub output_pct: f64,
    pub cache_creation_pct: f64,
    pub cache_read_pct: f64,
    // Cache analysis (Dimension 6)
    pub cache_hit_rate: f64,
    // Cost (Dimension 7)
    pub cost: CostBreakdown,
    pub total_cost_usd: f64,
    // Model breakdown (Dimension 3)
    pub model_breakdown: Vec<ModelBreakdown>,
    // Tool ranking (Dimension 8)
    pub tool_ranking: Vec<ToolUsageStat>,
    // Compression (Dimension 9)
    pub compression_events: Vec<CompressionEvent>,
    // Turn count
    pub total_turns: u64,
    pub avg_tokens_per_turn: f64,
}

/// Per-model cost and token breakdown within a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelBreakdown {
    pub model: String,
    pub turn_count: u64,
    pub total_tokens: u64,
    pub token_pct: f64,
    pub cost_usd: f64,
    pub cost_pct: f64,
}

/// Project-level analysis (from DB queries)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAnalysis {
    pub project_path: String,
    pub stats: ProjectStats,
    pub top_sessions: Vec<SessionRow>,
    pub tool_ranking: Vec<ToolRankEntry>,
    pub model_distribution: Vec<ModelDistEntry>,
    pub subagent_ratio: SubagentRatio,
}

/// Tool ranking entry with session count
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRankEntry {
    pub name: String,
    pub total_invocations: u64,
    pub session_count: u64,
}

/// Model distribution entry across sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDistEntry {
    pub model: String,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub session_count: u64,
}

/// Subagent vs main session ratio
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentRatio {
    pub main_sessions: u64,
    pub subagent_sessions: u64,
    pub main_tokens: u64,
    pub subagent_tokens: u64,
    pub subagent_token_pct: f64,
}

/// Global analysis (from DB)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalAnalysis {
    pub stats: GlobalStats,
    pub project_ranking: Vec<ProjectSummary>,
    pub top_sessions: Vec<SessionRow>,
    pub tool_ranking: Vec<ToolUsageStat>,
    pub subagent_ratio: SubagentRatio,
}

/// Trend analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendAnalysis {
    pub granularity: String,
    pub data_points: Vec<DailyStats>,
    pub total_days: u64,
    pub avg_daily_cost: f64,
    pub avg_daily_tokens: u64,
    pub peak_day: Option<DailyStats>,
}

/// Monthly cost report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostReport {
    pub month: String,
    pub total_cost: f64,
    pub daily_breakdown: Vec<DailyStats>,
    pub project_breakdown: Vec<ProjectSummary>,
    pub model_breakdown: Vec<ModelDistEntry>,
}

// === Analysis functions ===

/// Analyze a single session from its ParseResult
pub fn analyze_session(result: &ParseResult, pricing: &PricingTable) -> SessionAnalysis {
    let total_tokens = result.total_input_tokens
        + result.total_output_tokens
        + result.total_cache_creation_tokens
        + result.total_cache_read_tokens;

    // Calculate percentages (safe division)
    let (input_pct, output_pct, cache_creation_pct, cache_read_pct) = if total_tokens > 0 {
        let t = total_tokens as f64;
        (
            result.total_input_tokens as f64 / t * 100.0,
            result.total_output_tokens as f64 / t * 100.0,
            result.total_cache_creation_tokens as f64 / t * 100.0,
            result.total_cache_read_tokens as f64 / t * 100.0,
        )
    } else {
        (0.0, 0.0, 0.0, 0.0)
    };

    // Cache hit rate
    let cache_denominator = result.total_input_tokens
        + result.total_cache_read_tokens
        + result.total_cache_creation_tokens;
    let cache_hit_rate = if cache_denominator > 0 {
        result.total_cache_read_tokens as f64 / cache_denominator as f64
    } else {
        0.0
    };

    // Calculate per-model costs and build model breakdown
    let mut total_cost_usd = 0.0;
    let model_costs: Vec<(String, u64, u64, f64)> = result
        .model_usage
        .iter()
        .map(|mu| {
            let usage = TokenUsage {
                input_tokens: mu.total_input,
                output_tokens: mu.total_output,
                cache_creation_input_tokens: mu.total_cache_creation,
                cache_read_input_tokens: mu.total_cache_read,
            };
            let cost = pricing.calculate_cost(&mu.model, &usage);
            total_cost_usd += cost.total_cost;
            let model_total_tokens =
                mu.total_input + mu.total_output + mu.total_cache_creation + mu.total_cache_read;
            (
                mu.model.clone(),
                mu.turn_count,
                model_total_tokens,
                cost.total_cost,
            )
        })
        .collect();

    let model_breakdown: Vec<ModelBreakdown> = model_costs
        .iter()
        .map(|(model, turn_count, model_tokens, cost)| ModelBreakdown {
            model: model.clone(),
            turn_count: *turn_count,
            total_tokens: *model_tokens,
            token_pct: if total_tokens > 0 {
                *model_tokens as f64 / total_tokens as f64 * 100.0
            } else {
                0.0
            },
            cost_usd: *cost,
            cost_pct: if total_cost_usd > 0.0 {
                *cost / total_cost_usd * 100.0
            } else {
                0.0
            },
        })
        .collect();

    // Aggregate cost breakdown
    let cost = aggregate_cost_breakdown(result, pricing);

    // Duration range
    let duration_range = match (result.first_timestamp, result.last_timestamp) {
        (Some(first), Some(last)) => Some((first.to_rfc3339(), last.to_rfc3339())),
        _ => None,
    };

    // Average tokens per turn
    let avg_tokens_per_turn = if result.total_turns > 0 {
        total_tokens as f64 / result.total_turns as f64
    } else {
        0.0
    };

    SessionAnalysis {
        session_id: result.session_id.clone(),
        project_path: result.project_path.clone(),
        is_subagent: result.is_subagent,
        duration_range,
        total_tokens,
        input_tokens: result.total_input_tokens,
        output_tokens: result.total_output_tokens,
        cache_creation_tokens: result.total_cache_creation_tokens,
        cache_read_tokens: result.total_cache_read_tokens,
        input_pct,
        output_pct,
        cache_creation_pct,
        cache_read_pct,
        cache_hit_rate,
        cost,
        total_cost_usd,
        model_breakdown,
        tool_ranking: result.tool_usage.clone(),
        compression_events: result.compression_events.clone(),
        total_turns: result.total_turns,
        avg_tokens_per_turn,
    }
}

/// Calculate the aggregate cost breakdown for a session
fn aggregate_cost_breakdown(result: &ParseResult, pricing: &PricingTable) -> CostBreakdown {
    let mut input_cost = 0.0;
    let mut output_cost = 0.0;
    let mut cache_creation_cost = 0.0;
    let mut cache_read_cost = 0.0;

    for mu in &result.model_usage {
        let usage = TokenUsage {
            input_tokens: mu.total_input,
            output_tokens: mu.total_output,
            cache_creation_input_tokens: mu.total_cache_creation,
            cache_read_input_tokens: mu.total_cache_read,
        };
        let cb = pricing.calculate_cost(&mu.model, &usage);
        input_cost += cb.input_cost;
        output_cost += cb.output_cost;
        cache_creation_cost += cb.cache_creation_cost;
        cache_read_cost += cb.cache_read_cost;
    }

    CostBreakdown {
        input_cost,
        output_cost,
        cache_creation_cost,
        cache_read_cost,
        total_cost: input_cost + output_cost + cache_creation_cost + cache_read_cost,
    }
}

/// Analyze a project from DB (requires Database reference)
pub fn analyze_project(db: &Database, project_path: &str, limit: u32) -> Result<ProjectAnalysis> {
    let stats = db.project_stats(project_path)?;
    let top_sessions = db.top_sessions_by_cost_for_project(project_path, limit)?;

    // Tool ranking with session_count
    let tool_data = db.project_tool_ranking(project_path)?;
    let tool_ranking: Vec<ToolRankEntry> = tool_data
        .into_iter()
        .map(|(name, total_invocations, session_count)| ToolRankEntry {
            name,
            total_invocations,
            session_count,
        })
        .collect();

    // Model distribution
    let model_data = db.project_model_distribution(project_path)?;
    let model_distribution: Vec<ModelDistEntry> = model_data
        .into_iter()
        .map(
            |(model, total_tokens, total_cost, session_count)| ModelDistEntry {
                model,
                total_tokens,
                total_cost,
                session_count,
            },
        )
        .collect();

    // Subagent ratio
    let (main_sessions, subagent_sessions, main_tokens, subagent_tokens) =
        db.subagent_ratio(Some(project_path))?;
    let total_tokens = main_tokens + subagent_tokens;
    let subagent_token_pct = if total_tokens > 0 {
        subagent_tokens as f64 / total_tokens as f64 * 100.0
    } else {
        0.0
    };

    Ok(ProjectAnalysis {
        project_path: project_path.to_string(),
        stats,
        top_sessions,
        tool_ranking,
        model_distribution,
        subagent_ratio: SubagentRatio {
            main_sessions,
            subagent_sessions,
            main_tokens,
            subagent_tokens,
            subagent_token_pct,
        },
    })
}

/// Global cross-project analysis from DB
pub fn analyze_global(db: &Database) -> Result<GlobalAnalysis> {
    let stats = db.global_stats()?;
    let project_ranking = db.list_projects()?;
    let top_sessions = db.top_sessions_by_cost(20)?;
    let tool_ranking = db.global_tool_ranking()?;

    let (main_sessions, subagent_sessions, main_tokens, subagent_tokens) =
        db.subagent_ratio(None)?;
    let total_tokens = main_tokens + subagent_tokens;
    let subagent_token_pct = if total_tokens > 0 {
        subagent_tokens as f64 / total_tokens as f64 * 100.0
    } else {
        0.0
    };

    Ok(GlobalAnalysis {
        stats,
        project_ranking,
        top_sessions,
        tool_ranking,
        subagent_ratio: SubagentRatio {
            main_sessions,
            subagent_sessions,
            main_tokens,
            subagent_tokens,
            subagent_token_pct,
        },
    })
}

/// Trend analysis from DB
pub fn analyze_trend(
    db: &Database,
    project_path: Option<&str>,
    last_n_days: u32,
) -> Result<TrendAnalysis> {
    let data_points = db.daily_trend(project_path, last_n_days)?;

    let total_days = data_points.len() as u64;

    let total_cost: f64 = data_points.iter().map(|d| d.total_cost).sum();
    let total_tokens_sum: u64 = data_points
        .iter()
        .map(|d| d.total_input + d.total_output + d.total_cache_creation + d.total_cache_read)
        .sum();

    let avg_daily_cost = if total_days > 0 {
        total_cost / total_days as f64
    } else {
        0.0
    };

    let avg_daily_tokens = if total_days > 0 {
        total_tokens_sum / total_days
    } else {
        0
    };

    let peak_day = data_points
        .iter()
        .max_by(|a, b| {
            a.total_cost
                .partial_cmp(&b.total_cost)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned();

    Ok(TrendAnalysis {
        granularity: "daily".to_string(),
        data_points,
        total_days,
        avg_daily_cost,
        avg_daily_tokens,
        peak_day,
    })
}

/// Monthly cost report from DB
pub fn analyze_cost(db: &Database, month: &str, project_path: Option<&str>) -> Result<CostReport> {
    let daily_breakdown = db.daily_trend_for_month(month, project_path)?;
    let project_breakdown = db.list_projects_for_month(month, project_path)?;

    let model_data = db.model_distribution_for_month(month, project_path)?;
    let model_breakdown: Vec<ModelDistEntry> = model_data
        .into_iter()
        .map(
            |(model, total_tokens, total_cost, session_count)| ModelDistEntry {
                model,
                total_tokens,
                total_cost,
                session_count,
            },
        )
        .collect();

    let total_cost: f64 = daily_breakdown.iter().map(|d| d.total_cost).sum();

    Ok(CostReport {
        month: month.to_string(),
        total_cost,
        daily_breakdown,
        project_breakdown,
        model_breakdown,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ModelUsage, ToolUsageStat};
    use chrono::Utc;

    fn test_pricing() -> PricingTable {
        PricingTable::embedded()
    }

    /// Create a minimal ParseResult for testing
    fn make_test_parse_result(
        session_id: &str,
        project_path: &str,
        input: u64,
        output: u64,
        cache_creation: u64,
        cache_read: u64,
    ) -> ParseResult {
        let ts = "2026-03-20T10:00:00Z".parse::<chrono::DateTime<Utc>>().ok();

        ParseResult {
            session_id: session_id.to_string(),
            project_path: project_path.to_string(),
            is_subagent: false,
            agent_id: None,
            first_timestamp: ts,
            last_timestamp: ts,
            assistant_turns: vec![],
            model_usage: vec![ModelUsage {
                model: "claude-sonnet-4-20250514".to_string(),
                turn_count: 5,
                total_input: input,
                total_output: output,
                total_cache_creation: cache_creation,
                total_cache_read: cache_read,
            }],
            tool_usage: vec![
                ToolUsageStat {
                    name: "Read".to_string(),
                    invocation_count: 10,
                },
                ToolUsageStat {
                    name: "Bash".to_string(),
                    invocation_count: 5,
                },
            ],
            compression_events: vec![],
            total_turns: 5,
            total_input_tokens: input,
            total_output_tokens: output,
            total_cache_creation_tokens: cache_creation,
            total_cache_read_tokens: cache_read,
            failed_lines: 0,
            total_lines: 20,
            turn_durations_ms: vec![500, 1200],
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn make_db_parse_result(
        session_id: &str,
        project_path: &str,
        input: u64,
        output: u64,
        cache_creation: u64,
        cache_read: u64,
        timestamp_str: &str,
        is_subagent: bool,
    ) -> ParseResult {
        let ts = timestamp_str.parse::<chrono::DateTime<Utc>>().ok();

        ParseResult {
            session_id: session_id.to_string(),
            project_path: project_path.to_string(),
            is_subagent,
            agent_id: if is_subagent {
                Some("agent-test".to_string())
            } else {
                None
            },
            first_timestamp: ts,
            last_timestamp: ts,
            assistant_turns: vec![],
            model_usage: vec![ModelUsage {
                model: "claude-sonnet-4-20250514".to_string(),
                turn_count: 3,
                total_input: input,
                total_output: output,
                total_cache_creation: cache_creation,
                total_cache_read: cache_read,
            }],
            tool_usage: vec![
                ToolUsageStat {
                    name: "Read".to_string(),
                    invocation_count: 3,
                },
                ToolUsageStat {
                    name: "Bash".to_string(),
                    invocation_count: 2,
                },
            ],
            compression_events: vec![],
            total_turns: 3,
            total_input_tokens: input,
            total_output_tokens: output,
            total_cache_creation_tokens: cache_creation,
            total_cache_read_tokens: cache_read,
            failed_lines: 0,
            total_lines: 10,
            turn_durations_ms: vec![500],
        }
    }

    #[test]
    fn test_analyze_session_basic() {
        let pricing = test_pricing();
        let result = make_test_parse_result("sess-001", "/project/a", 1000, 500, 200, 800);
        let analysis = analyze_session(&result, &pricing);

        assert_eq!(analysis.session_id, "sess-001");
        assert_eq!(analysis.total_tokens, 2500); // 1000+500+200+800
        assert_eq!(analysis.input_tokens, 1000);
        assert_eq!(analysis.output_tokens, 500);

        // Percentage checks
        let epsilon = 0.1;
        assert!((analysis.input_pct - 40.0).abs() < epsilon); // 1000/2500 * 100
        assert!((analysis.output_pct - 20.0).abs() < epsilon); // 500/2500 * 100
        assert!((analysis.cache_creation_pct - 8.0).abs() < epsilon); // 200/2500 * 100
        assert!((analysis.cache_read_pct - 32.0).abs() < epsilon); // 800/2500 * 100

        // Cache hit rate = 800 / (1000 + 800 + 200) = 0.4
        assert!(
            (analysis.cache_hit_rate - 0.4).abs() < 0.001,
            "cache_hit_rate should be 0.4, got {}",
            analysis.cache_hit_rate
        );

        // Cost should be positive
        assert!(analysis.total_cost_usd > 0.0);

        // Model breakdown
        assert_eq!(analysis.model_breakdown.len(), 1);
        assert!((analysis.model_breakdown[0].token_pct - 100.0).abs() < epsilon);
        assert!((analysis.model_breakdown[0].cost_pct - 100.0).abs() < epsilon);

        // Avg tokens per turn
        assert!((analysis.avg_tokens_per_turn - 500.0).abs() < 0.1); // 2500/5

        // Duration range should be present
        assert!(analysis.duration_range.is_some());
    }

    #[test]
    fn test_analyze_session_zero_tokens() {
        let pricing = test_pricing();
        let result = make_test_parse_result("sess-zero", "/project/z", 0, 0, 0, 0);
        let analysis = analyze_session(&result, &pricing);

        assert_eq!(analysis.total_tokens, 0);
        assert_eq!(analysis.input_pct, 0.0);
        assert_eq!(analysis.output_pct, 0.0);
        assert_eq!(analysis.cache_creation_pct, 0.0);
        assert_eq!(analysis.cache_read_pct, 0.0);
        assert_eq!(analysis.cache_hit_rate, 0.0);
        assert_eq!(analysis.total_cost_usd, 0.0);
        assert_eq!(analysis.avg_tokens_per_turn, 0.0);
    }

    #[test]
    fn test_analyze_session_model_breakdown() {
        let pricing = test_pricing();
        let ts = "2026-03-20T10:00:00Z".parse::<chrono::DateTime<Utc>>().ok();

        let result = ParseResult {
            session_id: "sess-multi".to_string(),
            project_path: "/project/multi".to_string(),
            is_subagent: false,
            agent_id: None,
            first_timestamp: ts,
            last_timestamp: ts,
            assistant_turns: vec![],
            model_usage: vec![
                ModelUsage {
                    model: "claude-sonnet-4-20250514".to_string(),
                    turn_count: 3,
                    total_input: 500,
                    total_output: 200,
                    total_cache_creation: 100,
                    total_cache_read: 400,
                },
                ModelUsage {
                    model: "claude-opus-4-6".to_string(),
                    turn_count: 2,
                    total_input: 500,
                    total_output: 300,
                    total_cache_creation: 100,
                    total_cache_read: 400,
                },
            ],
            tool_usage: vec![],
            compression_events: vec![],
            total_turns: 5,
            total_input_tokens: 1000,
            total_output_tokens: 500,
            total_cache_creation_tokens: 200,
            total_cache_read_tokens: 800,
            failed_lines: 0,
            total_lines: 10,
            turn_durations_ms: vec![],
        };

        let analysis = analyze_session(&result, &pricing);

        assert_eq!(analysis.model_breakdown.len(), 2);

        // Both models have the same total tokens (1200 each), so 50%/50%
        let epsilon = 0.1;
        for mb in &analysis.model_breakdown {
            assert!(
                (mb.token_pct - 48.0).abs() < 5.0,
                "Each model should have roughly 48% of tokens, got {}",
                mb.token_pct
            );
        }

        // Opus should cost more (higher per-token pricing)
        let opus = analysis
            .model_breakdown
            .iter()
            .find(|m| m.model == "claude-opus-4-6")
            .unwrap();
        let sonnet = analysis
            .model_breakdown
            .iter()
            .find(|m| m.model == "claude-sonnet-4-20250514")
            .unwrap();
        assert!(
            opus.cost_usd > sonnet.cost_usd,
            "Opus should cost more than Sonnet"
        );

        // Cost percentages should sum to ~100%
        let total_cost_pct: f64 = analysis.model_breakdown.iter().map(|m| m.cost_pct).sum();
        assert!(
            (total_cost_pct - 100.0).abs() < epsilon,
            "Cost percentages should sum to 100%, got {}",
            total_cost_pct
        );
    }

    #[test]
    fn test_analyze_project() {
        let db = Database::open_in_memory().unwrap();
        let pricing = test_pricing();

        let r1 = make_db_parse_result(
            "sess-p1",
            "/project/x",
            1000,
            500,
            200,
            800,
            "2026-03-20T10:00:00Z",
            false,
        );
        let r2 = make_db_parse_result(
            "sess-p2",
            "/project/x",
            2000,
            1000,
            400,
            1600,
            "2026-03-20T11:00:00Z",
            false,
        );
        let r3 = make_db_parse_result(
            "sess-p3",
            "/project/x",
            500,
            250,
            100,
            400,
            "2026-03-20T12:00:00Z",
            true,
        );

        db.upsert_session(&r1, &pricing).unwrap();
        db.upsert_session(&r2, &pricing).unwrap();
        db.upsert_session(&r3, &pricing).unwrap();

        let analysis = analyze_project(&db, "/project/x", 10).unwrap();

        assert_eq!(analysis.project_path, "/project/x");
        assert_eq!(analysis.stats.session_count, 3);
        assert!(analysis.stats.total_cost_usd > 0.0);

        // Top sessions should be sorted by cost descending
        assert!(!analysis.top_sessions.is_empty());
        if analysis.top_sessions.len() >= 2 {
            assert!(
                analysis.top_sessions[0].total_cost_usd.unwrap_or(0.0)
                    >= analysis.top_sessions[1].total_cost_usd.unwrap_or(0.0)
            );
        }

        // Tool ranking
        assert!(!analysis.tool_ranking.is_empty());
        let read_tool = analysis.tool_ranking.iter().find(|t| t.name == "Read");
        assert!(read_tool.is_some());
        assert!(read_tool.unwrap().session_count >= 1);

        // Model distribution
        assert!(!analysis.model_distribution.is_empty());

        // Subagent ratio
        assert_eq!(analysis.subagent_ratio.main_sessions, 2);
        assert_eq!(analysis.subagent_ratio.subagent_sessions, 1);
        assert!(analysis.subagent_ratio.subagent_token_pct > 0.0);
        assert!(analysis.subagent_ratio.subagent_token_pct < 100.0);
    }

    #[test]
    fn test_analyze_global() {
        let db = Database::open_in_memory().unwrap();
        let pricing = test_pricing();

        let r1 = make_db_parse_result(
            "sess-g1",
            "/project/a",
            1000,
            500,
            200,
            800,
            "2026-03-20T10:00:00Z",
            false,
        );
        let r2 = make_db_parse_result(
            "sess-g2",
            "/project/b",
            2000,
            1000,
            400,
            1600,
            "2026-03-20T11:00:00Z",
            false,
        );
        let r3 = make_db_parse_result(
            "sess-g3",
            "/project/a",
            500,
            250,
            100,
            400,
            "2026-03-20T12:00:00Z",
            true,
        );

        db.upsert_session(&r1, &pricing).unwrap();
        db.upsert_session(&r2, &pricing).unwrap();
        db.upsert_session(&r3, &pricing).unwrap();

        let analysis = analyze_global(&db).unwrap();

        assert_eq!(analysis.stats.total_sessions, 3);
        assert_eq!(analysis.stats.total_projects, 2);
        assert!(analysis.stats.total_cost_usd > 0.0);

        // Project ranking
        assert_eq!(analysis.project_ranking.len(), 2);

        // Top sessions (limited to 20)
        assert!(analysis.top_sessions.len() <= 20);

        // Tool ranking
        assert!(!analysis.tool_ranking.is_empty());

        // Subagent ratio
        assert_eq!(analysis.subagent_ratio.main_sessions, 2);
        assert_eq!(analysis.subagent_ratio.subagent_sessions, 1);
    }

    #[test]
    fn test_analyze_trend() {
        let db = Database::open_in_memory().unwrap();
        let pricing = test_pricing();

        let r1 = make_db_parse_result(
            "sess-t1",
            "/project/t",
            1000,
            500,
            0,
            0,
            "2026-03-18T10:00:00Z",
            false,
        );
        let r2 = make_db_parse_result(
            "sess-t2",
            "/project/t",
            2000,
            1000,
            0,
            0,
            "2026-03-19T10:00:00Z",
            false,
        );
        let r3 = make_db_parse_result(
            "sess-t3",
            "/project/t",
            3000,
            1500,
            0,
            0,
            "2026-03-19T14:00:00Z",
            false,
        );

        db.upsert_session(&r1, &pricing).unwrap();
        db.upsert_session(&r2, &pricing).unwrap();
        db.upsert_session(&r3, &pricing).unwrap();

        let trend = analyze_trend(&db, Some("/project/t"), 365).unwrap();

        assert_eq!(trend.granularity, "daily");
        assert_eq!(trend.total_days, 2);
        assert!(trend.avg_daily_cost > 0.0);
        assert!(trend.avg_daily_tokens > 0);
        assert!(trend.peak_day.is_some());

        // Peak day should be 2026-03-19 (more sessions, higher cost)
        let peak = trend.peak_day.unwrap();
        assert_eq!(peak.date, "2026-03-19");
    }

    #[test]
    fn test_analyze_cost() {
        let db = Database::open_in_memory().unwrap();
        let pricing = test_pricing();

        let r1 = make_db_parse_result(
            "sess-c1",
            "/project/c",
            1000,
            500,
            0,
            0,
            "2026-03-18T10:00:00Z",
            false,
        );
        let r2 = make_db_parse_result(
            "sess-c2",
            "/project/c",
            2000,
            1000,
            0,
            0,
            "2026-03-19T10:00:00Z",
            false,
        );

        db.upsert_session(&r1, &pricing).unwrap();
        db.upsert_session(&r2, &pricing).unwrap();

        let report = analyze_cost(&db, "2026-03", None).unwrap();

        assert_eq!(report.month, "2026-03");
        assert!(report.total_cost > 0.0);
        assert!(!report.daily_breakdown.is_empty());
        assert!(!report.project_breakdown.is_empty());
        assert!(!report.model_breakdown.is_empty());
    }
}
