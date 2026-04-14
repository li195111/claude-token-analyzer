use std::collections::HashMap;

use crate::pattern_classifier::Signals;
use crate::types::{AssistantTurn, ParseResult};

const SEARCH_TOOLS: &[&str] = &["Read", "Grep", "Glob"];
const EDIT_TOOLS: &[&str] = &["Edit", "Write", "MultiEdit"];
const SUBAGENT_TOOL: &str = "Agent";

pub fn build_signals(parse_result: &ParseResult) -> Signals {
    Signals {
        cache_hit_rate: cache_hit_rate(parse_result),
        output_token_ratio: output_token_ratio(parse_result),
        subagent_count: count_tool_invocations(&parse_result.assistant_turns, SUBAGENT_TOOL),
        repeated_edit_peak: repeated_edit_peak(&parse_result.assistant_turns),
        turn_count: parse_result.total_turns as u32,
        duration_minutes: duration_minutes(parse_result),
        topic_shift_count: topic_shift_count(&parse_result.assistant_turns),
    }
}

fn cache_hit_rate(parse_result: &ParseResult) -> f64 {
    let denominator = parse_result.total_input_tokens
        + parse_result.total_cache_read_tokens
        + parse_result.total_cache_creation_tokens;
    if denominator == 0 {
        0.0
    } else {
        parse_result.total_cache_read_tokens as f64 / denominator as f64
    }
}

fn output_token_ratio(parse_result: &ParseResult) -> f64 {
    let denominator = parse_result.total_input_tokens + parse_result.total_output_tokens;
    if denominator == 0 {
        0.0
    } else {
        parse_result.total_output_tokens as f64 / denominator as f64
    }
}

fn count_tool_invocations(turns: &[AssistantTurn], target: &str) -> u32 {
    turns
        .iter()
        .flat_map(|turn| turn.tools.iter())
        .filter(|tool| tool.name == target)
        .count() as u32
}

fn repeated_edit_peak(turns: &[AssistantTurn]) -> u32 {
    let mut counts: HashMap<&str, u32> = HashMap::new();

    for tool in turns.iter().flat_map(|turn| turn.tools.iter()) {
        if !EDIT_TOOLS.contains(&tool.name.as_str()) {
            continue;
        }
        let Some(file_path) = tool.file_path.as_deref() else {
            continue;
        };
        *counts.entry(file_path).or_default() += 1;
    }

    counts.into_values().max().unwrap_or(0)
}

fn duration_minutes(parse_result: &ParseResult) -> Option<u32> {
    let first = parse_result.first_timestamp?;
    let last = parse_result.last_timestamp?;
    let minutes = last.signed_duration_since(first).num_minutes();
    Some(minutes.max(0) as u32)
}

fn topic_shift_count(turns: &[AssistantTurn]) -> u32 {
    if turns.len() < 3 {
        return 0;
    }

    let mut shifts = 0;
    for window in turns.windows(3) {
        let prev_prev = &window[0];
        let prev = &window[1];
        let current = &window[2];

        if is_search_only_turn(prev_prev) && is_search_only_turn(prev) && has_edit_tool(current) {
            shifts += 1;
        }
    }

    shifts
}

fn is_search_only_turn(turn: &AssistantTurn) -> bool {
    !turn.tools.is_empty()
        && turn
            .tools
            .iter()
            .all(|tool| SEARCH_TOOLS.contains(&tool.name.as_str()))
}

fn has_edit_tool(turn: &AssistantTurn) -> bool {
    turn.tools
        .iter()
        .any(|tool| EDIT_TOOLS.contains(&tool.name.as_str()))
}
