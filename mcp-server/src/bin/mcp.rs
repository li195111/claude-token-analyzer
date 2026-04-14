use std::path::{Path, PathBuf};

use rmcp::handler::server::tool::{ToolCallContext, ToolRouter};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::handler::server::ServerHandler;
use rmcp::model::*;
use rmcp::service::{RequestContext, ServiceExt};
use rmcp::{tool, tool_router, ErrorData as McpError, RoleServer};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::info;

use claude_token_analyzer::analyzer::{
    analyze_cost, analyze_global, analyze_project, analyze_session, analyze_trend,
};
use claude_token_analyzer::detector::detect_anomalies;
use claude_token_analyzer::parser::parse_jsonl_file;
use claude_token_analyzer::pattern_classifier::classify_with_validation;
use claude_token_analyzer::pattern_signals::build_signals;
use claude_token_analyzer::pricing::PricingTable;
use claude_token_analyzer::session_finder::{resolve_session_file, SessionLookupError};
use claude_token_analyzer::storage::Database;

// === Default value helpers ===

fn default_true() -> bool {
    true
}
fn default_sort_cost() -> String {
    "cost".to_string()
}
fn default_limit_10() -> u32 {
    10
}
fn default_stddev() -> f64 {
    2.0
}
fn default_min_tokens() -> u64 {
    10_000
}
fn default_daily() -> String {
    "daily".to_string()
}
fn default_30() -> u32 {
    30
}

// === Parameter structs ===

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AnalyzeSessionParams {
    /// Session ID (UUID) to analyze
    pub session_id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ClassifySessionPatternParams {
    /// Session ID (full UUID or unique prefix) to classify
    pub session_id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AnalyzeProjectParams {
    /// Project path to analyze (e.g., ~/.claude/projects/-Users-foo-myproject)
    pub project_path: String,
    /// Include subagent sessions in analysis (default: true)
    #[serde(default = "default_true")]
    pub include_subagents: bool,
    /// Sort sessions by: "cost", "tokens", "date" (default: "cost")
    #[serde(default = "default_sort_cost")]
    pub sort_by: String,
    /// Max number of top sessions to return (default: 10)
    #[serde(default = "default_limit_10")]
    pub limit: u32,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AnalyzeGlobalParams {}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CostReportParams {
    /// Month in YYYY-MM format (default: current month)
    pub month: Option<String>,
    /// Include daily breakdown (default: true)
    #[serde(default = "default_true")]
    pub daily: bool,
    /// Filter to specific project path
    pub project_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AnomalyScanParams {
    /// Standard deviation threshold for anomaly detection (default: 2.0)
    #[serde(default = "default_stddev")]
    pub stddev_threshold: f64,
    /// Filter to specific project path
    pub project_path: Option<String>,
    /// Minimum total tokens for a session to be included in cache hit rate analysis (default: 10000). Short sessions naturally have low cache hit rates; this filters out that noise.
    #[serde(default = "default_min_tokens")]
    pub min_tokens_for_cache_check: u64,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TrendReportParams {
    /// Granularity: "daily", "weekly", "monthly" (default: "daily")
    #[serde(default = "default_daily")]
    pub granularity: String,
    /// Number of days to look back (default: 30)
    #[serde(default = "default_30")]
    pub last_n_days: u32,
    /// Filter to specific project path
    pub project_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SyncDbParams {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PatternToolError {
    code: &'static str,
    message: String,
}

// === Server ===

#[derive(Clone)]
pub struct TokenAnalyzerServer {
    db_path: PathBuf,
    projects_dir: PathBuf,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl TokenAnalyzerServer {
    fn new(db_path: PathBuf, projects_dir: PathBuf) -> Self {
        Self {
            db_path,
            projects_dir,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        name = "analyze_session",
        description = "Analyze a single Claude Code session by its session ID. Returns detailed token usage breakdown, cost analysis, model breakdown, tool ranking, and compression events."
    )]
    async fn analyze_session_tool(
        &self,
        params: Parameters<AnalyzeSessionParams>,
    ) -> Result<CallToolResult, McpError> {
        let projects_dir = self.projects_dir.clone();
        let session_id = params.0.session_id.clone();

        let result = tokio::task::spawn_blocking(move || {
            let jsonl_path = find_session_file(&projects_dir, &session_id)
                .ok_or_else(|| format!("Session file not found for session_id: {}", session_id))?;

            let parse_result = parse_jsonl_file(&jsonl_path)
                .map_err(|e| format!("Failed to parse session file: {}", e))?;

            let pricing = PricingTable::from_env_or_embedded()
                .map_err(|e| format!("Failed to load pricing: {}", e))?;
            let analysis = analyze_session(&parse_result, &pricing);

            serde_json::to_string_pretty(&analysis)
                .map_err(|e| format!("Failed to serialize analysis: {}", e))
        })
        .await
        .map_err(|e| McpError {
            code: ErrorCode::INTERNAL_ERROR,
            message: format!("Task join error: {}", e).into(),
            data: None,
        })?;

        match result {
            Ok(json) => Ok(CallToolResult::success(vec![Content::text(json)])),
            Err(msg) => Err(McpError {
                code: ErrorCode::INTERNAL_ERROR,
                message: msg.into(),
                data: None,
            }),
        }
    }

    #[tool(
        name = "classify_session_pattern",
        description = "Classify a single Claude Code session into a usage pattern. Accepts a full session ID or unique prefix and returns pattern, signals, severity, and evidence."
    )]
    async fn classify_session_pattern_tool(
        &self,
        params: Parameters<ClassifySessionPatternParams>,
    ) -> Result<CallToolResult, McpError> {
        let projects_dir = self.projects_dir.clone();
        let session_id = params.0.session_id.clone();

        let result = tokio::task::spawn_blocking(move || {
            classify_session_pattern_json(&projects_dir, &session_id)
        })
        .await
        .map_err(|e| McpError {
            code: ErrorCode::INTERNAL_ERROR,
            message: format!("Task join error: {}", e).into(),
            data: None,
        })?;

        match result {
            Ok(json) => Ok(CallToolResult::success(vec![Content::text(json)])),
            Err(err) => Err(pattern_tool_error_to_mcp(err)),
        }
    }

    #[tool(
        name = "analyze_project",
        description = "Analyze all sessions for a specific project. Returns aggregate statistics, top sessions by cost, tool ranking, model distribution, and subagent ratio."
    )]
    async fn analyze_project_tool(
        &self,
        params: Parameters<AnalyzeProjectParams>,
    ) -> Result<CallToolResult, McpError> {
        let db_path = self.db_path.clone();
        let project_path = params.0.project_path.clone();
        let limit = params.0.limit;

        let result = tokio::task::spawn_blocking(move || {
            let db =
                Database::open(&db_path).map_err(|e| format!("Failed to open database: {}", e))?;

            let analysis = analyze_project(&db, &project_path, limit)
                .map_err(|e| format!("Failed to analyze project: {}", e))?;

            serde_json::to_string_pretty(&analysis)
                .map_err(|e| format!("Failed to serialize analysis: {}", e))
        })
        .await
        .map_err(|e| McpError {
            code: ErrorCode::INTERNAL_ERROR,
            message: format!("Task join error: {}", e).into(),
            data: None,
        })?;

        match result {
            Ok(json) => Ok(CallToolResult::success(vec![Content::text(json)])),
            Err(msg) => Err(McpError {
                code: ErrorCode::INTERNAL_ERROR,
                message: msg.into(),
                data: None,
            }),
        }
    }

    #[tool(
        name = "analyze_global",
        description = "Analyze all sessions across all projects. Returns global statistics, project ranking by cost, top sessions, tool ranking, and subagent ratio."
    )]
    async fn analyze_global_tool(
        &self,
        #[allow(unused_variables)] params: Parameters<AnalyzeGlobalParams>,
    ) -> Result<CallToolResult, McpError> {
        let db_path = self.db_path.clone();

        let result = tokio::task::spawn_blocking(move || {
            let db =
                Database::open(&db_path).map_err(|e| format!("Failed to open database: {}", e))?;

            let analysis =
                analyze_global(&db).map_err(|e| format!("Failed to analyze global: {}", e))?;

            serde_json::to_string_pretty(&analysis)
                .map_err(|e| format!("Failed to serialize analysis: {}", e))
        })
        .await
        .map_err(|e| McpError {
            code: ErrorCode::INTERNAL_ERROR,
            message: format!("Task join error: {}", e).into(),
            data: None,
        })?;

        match result {
            Ok(json) => Ok(CallToolResult::success(vec![Content::text(json)])),
            Err(msg) => Err(McpError {
                code: ErrorCode::INTERNAL_ERROR,
                message: msg.into(),
                data: None,
            }),
        }
    }

    #[tool(
        name = "cost_report",
        description = "Generate a monthly cost report. Returns total cost, daily breakdown, project breakdown, and model breakdown for the specified month."
    )]
    async fn cost_report_tool(
        &self,
        params: Parameters<CostReportParams>,
    ) -> Result<CallToolResult, McpError> {
        let db_path = self.db_path.clone();
        let month = params
            .0
            .month
            .clone()
            .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m").to_string());
        let project_path = params.0.project_path.clone();

        let result = tokio::task::spawn_blocking(move || {
            let db =
                Database::open(&db_path).map_err(|e| format!("Failed to open database: {}", e))?;

            let report = analyze_cost(&db, &month, project_path.as_deref())
                .map_err(|e| format!("Failed to generate cost report: {}", e))?;

            serde_json::to_string_pretty(&report)
                .map_err(|e| format!("Failed to serialize report: {}", e))
        })
        .await
        .map_err(|e| McpError {
            code: ErrorCode::INTERNAL_ERROR,
            message: format!("Task join error: {}", e).into(),
            data: None,
        })?;

        match result {
            Ok(json) => Ok(CallToolResult::success(vec![Content::text(json)])),
            Err(msg) => Err(McpError {
                code: ErrorCode::INTERNAL_ERROR,
                message: msg.into(),
                data: None,
            }),
        }
    }

    #[tool(
        name = "anomaly_scan",
        description = "Scan for anomalous sessions using statistical analysis. Detects high token usage, high cost, unusual model mix, excessive tool use, low cache hit rate, and cost-inefficient sessions (high cost + low cache). Supports min_tokens_for_cache_check to filter short-session noise, and severity scoring (cost * (1 - cache_hit_rate)) for prioritization."
    )]
    async fn anomaly_scan_tool(
        &self,
        params: Parameters<AnomalyScanParams>,
    ) -> Result<CallToolResult, McpError> {
        let db_path = self.db_path.clone();
        let stddev_threshold = params.0.stddev_threshold;
        let project_path = params.0.project_path.clone();
        let min_tokens_for_cache_check = params.0.min_tokens_for_cache_check;

        let result = tokio::task::spawn_blocking(move || {
            let db =
                Database::open(&db_path).map_err(|e| format!("Failed to open database: {}", e))?;

            let report = detect_anomalies(&db, stddev_threshold, project_path.as_deref(), min_tokens_for_cache_check)
                .map_err(|e| format!("Failed to detect anomalies: {}", e))?;

            serde_json::to_string_pretty(&report)
                .map_err(|e| format!("Failed to serialize report: {}", e))
        })
        .await
        .map_err(|e| McpError {
            code: ErrorCode::INTERNAL_ERROR,
            message: format!("Task join error: {}", e).into(),
            data: None,
        })?;

        match result {
            Ok(json) => Ok(CallToolResult::success(vec![Content::text(json)])),
            Err(msg) => Err(McpError {
                code: ErrorCode::INTERNAL_ERROR,
                message: msg.into(),
                data: None,
            }),
        }
    }

    #[tool(
        name = "trend_report",
        description = "Generate a token usage and cost trend report. Returns daily data points, averages, and peak day over the specified time period."
    )]
    async fn trend_report_tool(
        &self,
        params: Parameters<TrendReportParams>,
    ) -> Result<CallToolResult, McpError> {
        let db_path = self.db_path.clone();
        let last_n_days = params.0.last_n_days;
        let project_path = params.0.project_path.clone();

        let result = tokio::task::spawn_blocking(move || {
            let db =
                Database::open(&db_path).map_err(|e| format!("Failed to open database: {}", e))?;

            let report = analyze_trend(&db, project_path.as_deref(), last_n_days)
                .map_err(|e| format!("Failed to generate trend report: {}", e))?;

            serde_json::to_string_pretty(&report)
                .map_err(|e| format!("Failed to serialize report: {}", e))
        })
        .await
        .map_err(|e| McpError {
            code: ErrorCode::INTERNAL_ERROR,
            message: format!("Task join error: {}", e).into(),
            data: None,
        })?;

        match result {
            Ok(json) => Ok(CallToolResult::success(vec![Content::text(json)])),
            Err(msg) => Err(McpError {
                code: ErrorCode::INTERNAL_ERROR,
                message: msg.into(),
                data: None,
            }),
        }
    }

    #[tool(
        name = "sync_db",
        description = "Synchronize the database by scanning all JSONL session files under the configured projects directory (CTA_PROJECTS_DIR, or CLAUDE_CONFIG_DIR/projects, or ~/.claude/projects) and upserting new or modified sessions. Returns a sync report with counts of files scanned, synced, and failed."
    )]
    async fn sync_db_tool(
        &self,
        #[allow(unused_variables)] params: Parameters<SyncDbParams>,
    ) -> Result<CallToolResult, McpError> {
        let db_path = self.db_path.clone();
        let projects_dir = self.projects_dir.clone();

        let result = tokio::task::spawn_blocking(move || {
            let db =
                Database::open(&db_path).map_err(|e| format!("Failed to open database: {}", e))?;

            let pricing = PricingTable::from_env_or_embedded()
                .map_err(|e| format!("Failed to load pricing: {}", e))?;
            let report = db
                .sync_all(&projects_dir, &pricing, false)
                .map_err(|e| format!("Failed to sync database: {}", e))?;

            serde_json::to_string_pretty(&report)
                .map_err(|e| format!("Failed to serialize report: {}", e))
        })
        .await
        .map_err(|e| McpError {
            code: ErrorCode::INTERNAL_ERROR,
            message: format!("Task join error: {}", e).into(),
            data: None,
        })?;

        match result {
            Ok(json) => Ok(CallToolResult::success(vec![Content::text(json)])),
            Err(msg) => Err(McpError {
                code: ErrorCode::INTERNAL_ERROR,
                message: msg.into(),
                data: None,
            }),
        }
    }
}

impl ServerHandler for TokenAnalyzerServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.instructions = Some(
            "Claude Token Analyzer: Analyze Claude Code session token usage, costs, and trends."
                .into(),
        );
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        std::future::ready(Ok(ListToolsResult::with_all_items(
            self.tool_router.list_all(),
        )))
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        let tool_context = ToolCallContext::new(self, request, context);
        self.tool_router.call(tool_context)
    }
}

/// Delegate to shared session finder (returns Option, callers convert to MCP error).
fn find_session_file(projects_dir: &Path, session_id: &str) -> Option<PathBuf> {
    claude_token_analyzer::session_finder::find_session_file(projects_dir, session_id)
}

fn classify_session_pattern_json(
    projects_dir: &Path,
    session_id_or_prefix: &str,
) -> Result<String, PatternToolError> {
    let jsonl_path =
        resolve_session_file(projects_dir, session_id_or_prefix).map_err(session_lookup_error)?;
    let parse_result = parse_jsonl_file(&jsonl_path).map_err(|e| PatternToolError {
        code: "PARSE_FAILED",
        message: format!("Failed to parse session file: {e}"),
    })?;
    let signals = build_signals(&parse_result);
    let result = classify_with_validation(signals).map_err(|e| PatternToolError {
        code: "INSUFFICIENT_DATA",
        message: e.to_string(),
    })?;

    serde_json::to_string_pretty(&result).map_err(|e| PatternToolError {
        code: "INTERNAL_ERROR",
        message: format!("Failed to serialize pattern result: {e}"),
    })
}

fn session_lookup_error(error: SessionLookupError) -> PatternToolError {
    match error {
        SessionLookupError::NotFound => PatternToolError {
            code: "SESSION_NOT_FOUND",
            message: "Session file not found".to_string(),
        },
        SessionLookupError::Ambiguous(paths) => PatternToolError {
            code: "AMBIGUOUS_SESSION_ID",
            message: format!(
                "Session ID prefix matched multiple files: {}",
                paths
                    .iter()
                    .filter_map(|p| p.file_stem().and_then(|s| s.to_str()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        },
    }
}

fn pattern_tool_error_to_mcp(error: PatternToolError) -> McpError {
    let code = match error.code {
        "SESSION_NOT_FOUND" | "AMBIGUOUS_SESSION_ID" | "INSUFFICIENT_DATA" => {
            ErrorCode::INVALID_PARAMS
        }
        "PARSE_FAILED" => ErrorCode::INVALID_REQUEST,
        _ => ErrorCode::INTERNAL_ERROR,
    };

    McpError {
        code,
        message: format!("{}: {}", error.code, error.message).into(),
        data: Some(serde_json::json!({ "code": error.code })),
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn assistant_line(
        request_id: &str,
        timestamp: &str,
        input_tokens: u64,
        output_tokens: u64,
        cache_creation_input_tokens: u64,
        cache_read_input_tokens: u64,
        tools_json: &str,
    ) -> String {
        let content: serde_json::Value = serde_json::from_str(&format!(
            r#"[{{"type":"text","text":"turn"}},{tools_json}]"#
        ))
        .unwrap();

        serde_json::json!({
            "type": "assistant",
            "requestId": request_id,
            "timestamp": timestamp,
            "isSidechain": false,
            "message": {
                "model": "claude-sonnet-4-20250514",
                "id": format!("msg_{request_id}"),
                "role": "assistant",
                "content": content,
                "stop_reason": "tool_use",
                "usage": {
                    "input_tokens": input_tokens,
                    "output_tokens": output_tokens,
                    "cache_creation_input_tokens": cache_creation_input_tokens,
                    "cache_read_input_tokens": cache_read_input_tokens
                }
            }
        })
        .to_string()
    }

    fn write_session(projects_dir: &Path, session_id: &str, lines: &[String]) -> PathBuf {
        let file = projects_dir.join(format!("{session_id}.jsonl"));
        fs::write(&file, lines.join("\n")).unwrap();
        file
    }

    fn timestamp_for(offset_minutes: u32) -> String {
        let total_minutes = 6 * 60 + offset_minutes;
        let hour = total_minutes / 60;
        let minute = total_minutes % 60;
        format!("2026-03-20T{hour:02}:{minute:02}:00.000Z")
    }

    #[test]
    fn test_classify_session_pattern_json_supports_unique_prefix() {
        let dir = tempdir().unwrap();
        write_session(
            dir.path(),
            "abc12345-session",
            &[
                assistant_line(
                    "req_1",
                    "2026-03-20T06:00:00.000Z",
                    100,
                    50,
                    0,
                    200,
                    r#"{"type":"tool_use","id":"toolu_1","name":"Read","input":{"file_path":"/src/lib.rs"}}"#,
                ),
                assistant_line(
                    "req_2",
                    "2026-03-20T06:01:00.000Z",
                    100,
                    50,
                    0,
                    200,
                    r#"{"type":"tool_use","id":"toolu_2","name":"Grep","input":{"pattern":"todo"}}"#,
                ),
                assistant_line(
                    "req_3",
                    "2026-03-20T06:02:00.000Z",
                    100,
                    50,
                    0,
                    200,
                    r#"{"type":"tool_use","id":"toolu_3","name":"Read","input":{"file_path":"/src/main.rs"}}"#,
                ),
            ],
        );

        let json = classify_session_pattern_json(dir.path(), "abc12345").unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["pattern"], "observer");
        assert_eq!(value["severity"], "info");
        assert_eq!(value["signals"]["turn_count"], 3);
    }

    #[test]
    fn test_classify_session_pattern_json_returns_cold_session() {
        let dir = tempdir().unwrap();
        write_session(
            dir.path(),
            "cold-session",
            &[
                assistant_line(
                    "req_1",
                    &timestamp_for(0),
                    200,
                    20,
                    0,
                    0,
                    r#"{"type":"tool_use","id":"toolu_1","name":"Read","input":{"file_path":"/src/lib.rs"}}"#,
                ),
                assistant_line(
                    "req_2",
                    &timestamp_for(1),
                    200,
                    20,
                    0,
                    0,
                    r#"{"type":"tool_use","id":"toolu_2","name":"Read","input":{"file_path":"/src/main.rs"}}"#,
                ),
                assistant_line(
                    "req_3",
                    &timestamp_for(2),
                    200,
                    20,
                    0,
                    0,
                    r#"{"type":"tool_use","id":"toolu_3","name":"Grep","input":{"pattern":"todo"}}"#,
                ),
            ],
        );

        let json = classify_session_pattern_json(dir.path(), "cold-session").unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["pattern"], "cold_session");
        assert_eq!(value["severity"], "alert");
    }

    #[test]
    fn test_classify_session_pattern_json_returns_correction_spiral() {
        let dir = tempdir().unwrap();
        write_session(
            dir.path(),
            "spiral-session",
            &[
                assistant_line(
                    "req_1",
                    &timestamp_for(0),
                    50,
                    45,
                    0,
                    50,
                    r#"{"type":"tool_use","id":"toolu_1","name":"Edit","input":{"file_path":"/src/lib.rs"}}"#,
                ),
                assistant_line(
                    "req_2",
                    &timestamp_for(1),
                    50,
                    45,
                    0,
                    50,
                    r#"{"type":"tool_use","id":"toolu_2","name":"Write","input":{"file_path":"/src/lib.rs"}}"#,
                ),
                assistant_line(
                    "req_3",
                    &timestamp_for(2),
                    50,
                    45,
                    0,
                    50,
                    r#"{"type":"tool_use","id":"toolu_3","name":"MultiEdit","input":{"file_path":"/src/lib.rs"}}"#,
                ),
                assistant_line(
                    "req_4",
                    &timestamp_for(3),
                    50,
                    45,
                    0,
                    50,
                    r#"{"type":"tool_use","id":"toolu_4","name":"Edit","input":{"file_path":"/src/lib.rs"}}"#,
                ),
            ],
        );

        let json = classify_session_pattern_json(dir.path(), "spiral-session").unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["pattern"], "correction_spiral");
        assert_eq!(value["severity"], "warn");
        assert_eq!(value["signals"]["repeated_edit_peak"], 4);
    }

    #[test]
    fn test_classify_session_pattern_json_returns_subagent_swarm() {
        let dir = tempdir().unwrap();
        let lines: Vec<String> = (0..11)
            .map(|i| {
                assistant_line(
                    &format!("req_{}", i + 1),
                    &timestamp_for(i),
                    100,
                    30,
                    0,
                    200,
                    r#"{"type":"tool_use","id":"toolu_agent","name":"Agent","input":{"task":"review"}}"#,
                )
            })
            .collect();
        write_session(dir.path(), "swarm-session", &lines);

        let json = classify_session_pattern_json(dir.path(), "swarm-session").unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["pattern"], "subagent_swarm");
        assert_eq!(value["severity"], "warn");
        assert_eq!(value["signals"]["subagent_count"], 11);
    }

    #[test]
    fn test_classify_session_pattern_json_returns_kitchen_sink() {
        let dir = tempdir().unwrap();
        let tool_payloads = [
            r#"{"type":"tool_use","id":"toolu_1","name":"Read","input":{"file_path":"/src/a.rs"}}"#,
            r#"{"type":"tool_use","id":"toolu_2","name":"Grep","input":{"pattern":"alpha"}}"#,
            r#"{"type":"tool_use","id":"toolu_3","name":"Edit","input":{"file_path":"/src/a.rs"}}"#,
            r#"{"type":"tool_use","id":"toolu_4","name":"Read","input":{"file_path":"/src/b.rs"}}"#,
            r#"{"type":"tool_use","id":"toolu_5","name":"Glob","input":{"pattern":"*.rs"}}"#,
            r#"{"type":"tool_use","id":"toolu_6","name":"Edit","input":{"file_path":"/src/b.rs"}}"#,
            r#"{"type":"tool_use","id":"toolu_7","name":"Read","input":{"file_path":"/src/c.rs"}}"#,
            r#"{"type":"tool_use","id":"toolu_8","name":"Grep","input":{"pattern":"beta"}}"#,
            r#"{"type":"tool_use","id":"toolu_9","name":"Write","input":{"file_path":"/src/a.rs"}}"#,
            r#"{"type":"tool_use","id":"toolu_10","name":"Read","input":{"file_path":"/src/d.rs"}}"#,
            r#"{"type":"tool_use","id":"toolu_11","name":"Glob","input":{"pattern":"*.md"}}"#,
            r#"{"type":"tool_use","id":"toolu_12","name":"MultiEdit","input":{"file_path":"/src/b.rs"}}"#,
        ];
        let lines: Vec<String> = tool_payloads
            .iter()
            .enumerate()
            .map(|(i, payload)| {
                assistant_line(
                    &format!("req_{}", i + 1),
                    &timestamp_for(i as u32),
                    100,
                    20,
                    0,
                    200,
                    payload,
                )
            })
            .collect();
        write_session(dir.path(), "kitchen-session", &lines);

        let json = classify_session_pattern_json(dir.path(), "kitchen-session").unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["pattern"], "kitchen_sink");
        assert_eq!(value["severity"], "info");
        assert_eq!(value["signals"]["topic_shift_count"], 4);
        assert_eq!(value["signals"]["repeated_edit_peak"], 2);
    }

    #[test]
    fn test_classify_session_pattern_json_returns_marathon() {
        let dir = tempdir().unwrap();
        let lines: Vec<String> = (0..61)
            .map(|i| {
                assistant_line(
                    &format!("req_{}", i + 1),
                    &timestamp_for(i * 2),
                    50,
                    20,
                    0,
                    200,
                    r#"{"type":"tool_use","id":"toolu_read","name":"Read","input":{"file_path":"/src/lib.rs"}}"#,
                )
            })
            .collect();
        write_session(dir.path(), "marathon-session", &lines);

        let json = classify_session_pattern_json(dir.path(), "marathon-session").unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["pattern"], "marathon");
        assert_eq!(value["severity"], "info");
        assert_eq!(value["signals"]["duration_minutes"], 120);
    }

    #[test]
    fn test_classify_session_pattern_json_returns_normal() {
        let dir = tempdir().unwrap();
        write_session(
            dir.path(),
            "normal-session",
            &[
                assistant_line(
                    "req_1",
                    &timestamp_for(0),
                    120,
                    30,
                    0,
                    120,
                    r#"{"type":"tool_use","id":"toolu_1","name":"Read","input":{"file_path":"/src/a.rs"}}"#,
                ),
                assistant_line(
                    "req_2",
                    &timestamp_for(1),
                    120,
                    30,
                    0,
                    120,
                    r#"{"type":"tool_use","id":"toolu_2","name":"Edit","input":{"file_path":"/src/a.rs"}}"#,
                ),
                assistant_line(
                    "req_3",
                    &timestamp_for(2),
                    120,
                    30,
                    0,
                    120,
                    r#"{"type":"tool_use","id":"toolu_3","name":"Read","input":{"file_path":"/src/b.rs"}}"#,
                ),
                assistant_line(
                    "req_4",
                    &timestamp_for(3),
                    120,
                    30,
                    0,
                    120,
                    r#"{"type":"tool_use","id":"toolu_4","name":"Grep","input":{"pattern":"gamma"}}"#,
                ),
                assistant_line(
                    "req_5",
                    &timestamp_for(4),
                    120,
                    30,
                    0,
                    120,
                    r#"{"type":"tool_use","id":"toolu_5","name":"Write","input":{"file_path":"/src/a.rs"}}"#,
                ),
            ],
        );

        let json = classify_session_pattern_json(dir.path(), "normal-session").unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["pattern"], "normal");
        assert_eq!(value["severity"], "info");
        assert_eq!(value["signals"]["repeated_edit_peak"], 2);
    }

    #[test]
    fn test_classify_session_pattern_json_returns_not_found_error() {
        let dir = tempdir().unwrap();
        let error = classify_session_pattern_json(dir.path(), "missing").unwrap_err();
        assert_eq!(error.code, "SESSION_NOT_FOUND");
    }

    #[test]
    fn test_classify_session_pattern_json_returns_ambiguous_error() {
        let dir = tempdir().unwrap();
        write_session(dir.path(), "abc12345-one", &[]);
        write_session(dir.path(), "abc12345-two", &[]);

        let error = classify_session_pattern_json(dir.path(), "abc12345").unwrap_err();
        assert_eq!(error.code, "AMBIGUOUS_SESSION_ID");
    }

    #[test]
    fn test_classify_session_pattern_json_returns_parse_failed_error() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("broken.jsonl");
        fs::write(&file, "{\n{\n{\n").unwrap();

        let error = classify_session_pattern_json(dir.path(), "broken").unwrap_err();
        assert_eq!(error.code, "PARSE_FAILED");
    }

    #[test]
    fn test_classify_session_pattern_json_returns_insufficient_data_error() {
        let dir = tempdir().unwrap();
        write_session(
            dir.path(),
            "short-session",
            &[
                assistant_line(
                    "req_1",
                    "2026-03-20T06:00:00.000Z",
                    100,
                    50,
                    0,
                    200,
                    r#"{"type":"tool_use","id":"toolu_1","name":"Read","input":{"file_path":"/src/lib.rs"}}"#,
                ),
                assistant_line(
                    "req_2",
                    "2026-03-20T06:01:00.000Z",
                    100,
                    50,
                    0,
                    200,
                    r#"{"type":"tool_use","id":"toolu_2","name":"Read","input":{"file_path":"/src/main.rs"}}"#,
                ),
            ],
        );

        let error = classify_session_pattern_json(dir.path(), "short-session").unwrap_err();
        assert_eq!(error.code, "INSUFFICIENT_DATA");
        assert!(error.message.contains("assistant turn(s), minimum is 3"));
    }

    #[test]
    fn test_pattern_tool_error_to_mcp_includes_symbolic_code_in_data() {
        let error = pattern_tool_error_to_mcp(PatternToolError {
            code: "SESSION_NOT_FOUND",
            message: "Session file not found".to_string(),
        });

        assert_eq!(error.code, ErrorCode::INVALID_PARAMS);
        assert_eq!(error.message.as_ref(), "SESSION_NOT_FOUND: Session file not found");
        assert_eq!(error.data, Some(serde_json::json!({ "code": "SESSION_NOT_FOUND" })));
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing to stderr (CRITICAL: stdout is MCP JSON-RPC channel)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let db_path = claude_token_analyzer::config::resolve_db_path()?;
    let projects_dir = claude_token_analyzer::config::resolve_projects_dir()?;

    info!(
        db = %db_path.display(),
        projects = %projects_dir.display(),
        "Starting Claude Token Analyzer MCP server"
    );

    let server = TokenAnalyzerServer::new(db_path, projects_dir);
    let service = server.serve(rmcp::transport::io::stdio()).await?;
    service.waiting().await?;

    Ok(())
}
