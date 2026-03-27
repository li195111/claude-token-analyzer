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
use claude_token_analyzer::pricing::PricingTable;
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
        description = "Synchronize the database by scanning all JSONL session files under the configured projects directory (default: ~/.claude/projects/, overridable via $CTA_PROJECTS_DIR) and upserting new or modified sessions. Returns a sync report with counts of files scanned, synced, and failed."
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
