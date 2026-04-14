use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// === RAW PARSE LAYER (zero computation) ===

/// Token usage from a single assistant turn
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub cache_read_input_tokens: u64,
}

/// A single tool invocation within an assistant turn
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseInfo {
    pub name: String,
    pub tool_use_id: String,
    pub file_path: Option<String>,
}

/// A single assistant turn (one model response)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantTurn {
    pub request_id: String,
    pub model: String,
    pub usage: TokenUsage,
    pub tools: Vec<ToolUseInfo>,
    pub stop_reason: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub is_sidechain: bool,
}

/// Model usage aggregation (per-model within a session)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUsage {
    pub model: String,
    pub turn_count: u64,
    pub total_input: u64,
    pub total_output: u64,
    pub total_cache_creation: u64,
    pub total_cache_read: u64,
}

/// Tool usage aggregation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUsageStat {
    pub name: String,
    pub invocation_count: u64,
}

/// Context compression event detected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionEvent {
    pub turn_index: usize,
    pub timestamp: DateTime<Utc>,
    pub cache_read_before: u64,
    pub cache_read_after: u64,
    pub drop_percentage: f64,
}

/// Complete parse result from a single JSONL file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseResult {
    pub session_id: String,
    pub project_path: String,
    pub is_subagent: bool,
    pub agent_id: Option<String>,
    pub first_timestamp: Option<DateTime<Utc>>,
    pub last_timestamp: Option<DateTime<Utc>>,
    pub assistant_turns: Vec<AssistantTurn>,
    pub model_usage: Vec<ModelUsage>,
    pub tool_usage: Vec<ToolUsageStat>,
    pub compression_events: Vec<CompressionEvent>,
    pub total_turns: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_creation_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub failed_lines: u64,
    pub total_lines: u64,
    pub turn_durations_ms: Vec<u64>,
}
