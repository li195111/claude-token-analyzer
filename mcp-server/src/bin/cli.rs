use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};

use claude_token_analyzer::analyzer::{
    analyze_cost, analyze_global, analyze_project, analyze_session, analyze_trend,
};
use claude_token_analyzer::archiver::Archiver;
use claude_token_analyzer::detector::detect_anomalies;
use claude_token_analyzer::parser::parse_jsonl_file;
use claude_token_analyzer::pricing::PricingTable;
use claude_token_analyzer::storage::Database;

#[derive(Parser)]
#[command(
    name = "cta",
    about = "Claude Token Analyzer — Analyze Claude Code session token usage"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Sync JSONL session files to SQLite database
    Sync {
        /// Show verbose output during sync
        #[arg(long)]
        verbose: bool,
    },

    /// Analyze sessions
    Analyze {
        /// Analyze a specific session by ID
        #[arg(long)]
        session: Option<String>,
        /// Analyze a specific project by path
        #[arg(long)]
        project: Option<String>,
        /// Analyze all projects globally
        #[arg(long)]
        global: bool,
    },

    /// Generate cost report
    Cost {
        /// Month in YYYY-MM format (default: current month)
        #[arg(long)]
        month: Option<String>,
        /// Show daily breakdown
        #[arg(long)]
        daily: bool,
        /// Filter to specific project
        #[arg(long)]
        project: Option<String>,
    },

    /// Archive expiring JSONL files with zstd compression
    Archive {
        /// Only show what would be archived, don't actually do it
        #[arg(long)]
        dry_run: bool,
        /// Days until expiration threshold (default: 25)
        #[arg(long, default_value = "25")]
        days_threshold: u32,
    },

    /// Export data in CSV or JSON format
    Export {
        /// Output format: csv or json
        #[arg(long, default_value = "json")]
        format: String,
        /// Output file path (default: stdout)
        #[arg(long, short)]
        output: Option<String>,
        /// Filter to specific project
        #[arg(long)]
        project: Option<String>,
    },

    /// Detect anomalous sessions
    Anomalies {
        /// Standard deviation threshold (default: 2.0)
        #[arg(long, default_value = "2.0")]
        threshold: f64,
        /// Filter to specific project
        #[arg(long)]
        project: Option<String>,
    },

    /// Show token usage trends
    Trend {
        /// Granularity: daily, weekly, monthly
        #[arg(long, default_value = "daily")]
        granularity: String,
        /// Number of days to look back
        #[arg(long, default_value = "30")]
        days: u32,
        /// Filter to specific project
        #[arg(long)]
        project: Option<String>,
    },
}

// === Path helpers (delegated to config module) ===

fn open_db() -> Result<Database> {
    let db_path = claude_token_analyzer::config::resolve_db_path()?;
    Database::open(&db_path)
}

use claude_token_analyzer::format::{
    csv_escape, fmt_cost, fmt_count, fmt_pct, fmt_tokens, pad, short_project_name,
    truncate_session_id,
};

// === Command implementations ===

fn cmd_sync(verbose: bool) -> Result<()> {
    let db = open_db()?;
    let pricing = PricingTable::from_env_or_embedded()?;
    let projects_dir = claude_token_analyzer::config::resolve_projects_dir()?;

    if verbose {
        println!("Syncing from: {}", projects_dir.display());
        println!("Database: {}", claude_token_analyzer::config::resolve_db_path()?.display());
    }

    let report = db.sync_all(&projects_dir, &pricing, verbose)?;

    println!("=== Sync Complete ===");
    println!("Total JSONL files: {}", fmt_count(report.total_files_on_disk));
    println!("New/modified:      {}", fmt_count(report.files_to_sync));
    println!("Synced:            {}", fmt_count(report.files_synced));
    println!("Failed:            {}", fmt_count(report.files_failed));
    println!("Sessions upserted: {}", fmt_count(report.sessions_upserted));

    Ok(())
}

fn cmd_analyze_session(session_id: &str) -> Result<()> {
    let pricing = PricingTable::from_env_or_embedded()?;
    let projects_dir = claude_token_analyzer::config::resolve_projects_dir()?;

    // Search for the JSONL file matching session_id
    let jsonl_path = find_session_file(&projects_dir, session_id)?;
    let result = parse_jsonl_file(&jsonl_path)?;
    let analysis = analyze_session(&result, &pricing);

    println!(
        "=== Claude Token Analyzer: Session {} ===",
        truncate_session_id(&analysis.session_id, 24)
    );
    println!();
    println!("Project: {}", analysis.project_path);
    println!(
        "Subagent: {}",
        if analysis.is_subagent { "Yes" } else { "No" }
    );
    if let Some((start, end)) = &analysis.duration_range {
        println!("Time range: {} -> {}", start, end);
    }
    println!("Turns: {}", analysis.total_turns);
    println!("Avg tokens/turn: {:.0}", analysis.avg_tokens_per_turn);
    println!();

    println!("--- Token Breakdown ---");
    println!("Total tokens:   {}", fmt_tokens(analysis.total_tokens));
    println!(
        "  Input:        {} ({})",
        fmt_tokens(analysis.input_tokens),
        fmt_pct(analysis.input_pct)
    );
    println!(
        "  Output:       {} ({})",
        fmt_tokens(analysis.output_tokens),
        fmt_pct(analysis.output_pct)
    );
    println!(
        "  Cache create: {} ({})",
        fmt_tokens(analysis.cache_creation_tokens),
        fmt_pct(analysis.cache_creation_pct)
    );
    println!(
        "  Cache read:   {} ({})",
        fmt_tokens(analysis.cache_read_tokens),
        fmt_pct(analysis.cache_read_pct)
    );
    println!(
        "Cache hit rate: {}",
        fmt_pct(analysis.cache_hit_rate * 100.0)
    );
    println!();

    println!("--- Cost ---");
    println!("Total cost: {}", fmt_cost(analysis.total_cost_usd));
    println!("  Input:        {}", fmt_cost(analysis.cost.input_cost));
    println!("  Output:       {}", fmt_cost(analysis.cost.output_cost));
    println!(
        "  Cache create: {}",
        fmt_cost(analysis.cost.cache_creation_cost)
    );
    println!(
        "  Cache read:   {}",
        fmt_cost(analysis.cost.cache_read_cost)
    );
    println!();

    if !analysis.model_breakdown.is_empty() {
        println!("--- Model Breakdown ---");
        for mb in &analysis.model_breakdown {
            println!(
                "  {}: {} turns, {} tokens ({}), {} ({})",
                mb.model,
                mb.turn_count,
                fmt_tokens(mb.total_tokens),
                fmt_pct(mb.token_pct),
                fmt_cost(mb.cost_usd),
                fmt_pct(mb.cost_pct)
            );
        }
        println!();
    }

    if !analysis.tool_ranking.is_empty() {
        println!("--- Tool Ranking ---");
        for (i, tool) in analysis.tool_ranking.iter().enumerate() {
            println!(
                "  {}. {:20} {} invocations",
                i + 1,
                tool.name,
                fmt_count(tool.invocation_count)
            );
        }
        println!();
    }

    if !analysis.compression_events.is_empty() {
        println!("--- Compression Events ---");
        for event in &analysis.compression_events {
            println!(
                "  Turn {}: {} -> {} ({:.0}% drop)",
                event.turn_index,
                fmt_tokens(event.cache_read_before),
                fmt_tokens(event.cache_read_after),
                event.drop_percentage * 100.0
            );
        }
    }

    Ok(())
}

fn cmd_analyze_project(project_path: &str) -> Result<()> {
    let db = open_db()?;
    let analysis = analyze_project(&db, project_path, 10)?;

    println!("=== Claude Token Analyzer: Project Report ===\n");
    println!("Project: {}", analysis.project_path);
    println!("Sessions: {}", fmt_count(analysis.stats.session_count));
    println!("Total tokens: {}", fmt_tokens(analysis.stats.total_tokens));
    println!("Total cost: {}", fmt_cost(analysis.stats.total_cost_usd));
    println!(
        "Avg cache hit rate: {}",
        fmt_pct(analysis.stats.avg_cache_hit_rate * 100.0)
    );
    println!();

    println!("--- Subagent Ratio ---");
    println!(
        "Main sessions: {}  Subagent sessions: {}",
        analysis.subagent_ratio.main_sessions, analysis.subagent_ratio.subagent_sessions
    );
    println!(
        "Subagent token share: {}",
        fmt_pct(analysis.subagent_ratio.subagent_token_pct)
    );
    println!();

    if !analysis.top_sessions.is_empty() {
        println!("--- Top Sessions (by cost) ---");
        for (i, s) in analysis.top_sessions.iter().enumerate() {
            let cost = s.total_cost_usd.unwrap_or(0.0);
            println!(
                "  {}. {}  {}  ({} turns)",
                i + 1,
                truncate_session_id(&s.session_id, 24),
                fmt_cost(cost),
                s.total_turns
            );
        }
        println!();
    }

    if !analysis.tool_ranking.is_empty() {
        println!("--- Tool Ranking ---");
        for (i, tool) in analysis.tool_ranking.iter().take(15).enumerate() {
            println!(
                "  {}. {:20} {} invocations ({} sessions)",
                i + 1,
                tool.name,
                fmt_count(tool.total_invocations),
                tool.session_count
            );
        }
        println!();
    }

    if !analysis.model_distribution.is_empty() {
        println!("--- Model Distribution ---");
        for md in &analysis.model_distribution {
            println!(
                "  {}: {} tokens, {} ({} sessions)",
                md.model,
                fmt_tokens(md.total_tokens),
                fmt_cost(md.total_cost),
                md.session_count
            );
        }
    }

    Ok(())
}

fn cmd_analyze_global() -> Result<()> {
    let db = open_db()?;
    let analysis = analyze_global(&db)?;

    println!("=== Claude Token Analyzer: Global Report ===\n");
    println!(
        "Sessions: {} across {} projects",
        fmt_count(analysis.stats.total_sessions),
        analysis.stats.total_projects
    );
    println!("Total tokens: {}", fmt_tokens(analysis.stats.total_tokens));
    println!("Total cost: {}", fmt_cost(analysis.stats.total_cost_usd));
    println!(
        "Avg cache hit rate: {}",
        fmt_pct(analysis.stats.avg_cache_hit_rate * 100.0)
    );
    println!();

    println!("--- Subagent Ratio ---");
    println!(
        "Main sessions: {}  Subagent sessions: {}",
        analysis.subagent_ratio.main_sessions, analysis.subagent_ratio.subagent_sessions
    );
    println!(
        "Subagent token share: {}",
        fmt_pct(analysis.subagent_ratio.subagent_token_pct)
    );
    println!();

    if !analysis.project_ranking.is_empty() {
        println!("--- Top Projects (by cost) ---");
        println!(
            "  {:>3}  {:<40} {:>10} {:>10}",
            "#", "Project", "Cost", "Sessions"
        );
        println!("  {}", "─".repeat(67));
        for (i, p) in analysis.project_ranking.iter().take(20).enumerate() {
            println!(
                "  {:>3}  {:<40} {:>10} {:>10}",
                i + 1,
                pad(&short_project_name(&p.project_path), 40),
                fmt_cost(p.total_cost_usd),
                fmt_count(p.session_count)
            );
        }
        println!();
    }

    if !analysis.top_sessions.is_empty() {
        println!("--- Top Sessions (by cost) ---");
        println!(
            "  {:>3}  {:<24} {:>10} {:>10}",
            "#", "Session", "Cost", "Turns"
        );
        println!("  {}", "─".repeat(51));
        for (i, s) in analysis.top_sessions.iter().take(10).enumerate() {
            let cost = s.total_cost_usd.unwrap_or(0.0);
            println!(
                "  {:>3}  {:<24} {:>10} {:>10}",
                i + 1,
                truncate_session_id(&s.session_id, 24),
                fmt_cost(cost),
                s.total_turns
            );
        }
        println!();
    }

    if !analysis.tool_ranking.is_empty() {
        println!("--- Tool Ranking ---");
        println!(
            "  {:>3}  {:<20} {:>12}",
            "#", "Tool", "Invocations"
        );
        println!("  {}", "─".repeat(39));
        for (i, tool) in analysis.tool_ranking.iter().take(15).enumerate() {
            println!(
                "  {:>3}  {:<20} {:>12}",
                i + 1,
                tool.name,
                fmt_count(tool.invocation_count)
            );
        }
    }

    Ok(())
}

fn cmd_cost(month: Option<String>, daily: bool, project: Option<String>) -> Result<()> {
    let db = open_db()?;
    let month_str = month.unwrap_or_else(|| Utc::now().format("%Y-%m").to_string());
    let project_ref = project.as_deref();

    let report = analyze_cost(&db, &month_str, project_ref)?;

    println!("=== Claude Token Analyzer: Cost Report ===\n");
    println!("Month: {}", report.month);
    if let Some(p) = &project {
        println!("Project: {}", p);
    }
    println!("Total cost: {}", fmt_cost(report.total_cost));
    println!();

    if !report.project_breakdown.is_empty() {
        println!("--- Project Breakdown ---");
        println!(
            "  {:>3}  {:<40} {:>10} {:>10}",
            "#", "Project", "Cost", "Sessions"
        );
        println!("  {}", "─".repeat(67));
        for (i, p) in report.project_breakdown.iter().enumerate() {
            println!(
                "  {:>3}  {:<40} {:>10} {:>10}",
                i + 1,
                pad(&short_project_name(&p.project_path), 40),
                fmt_cost(p.total_cost_usd),
                fmt_count(p.session_count)
            );
        }
        println!();
    }

    if !report.model_breakdown.is_empty() {
        println!("--- Model Breakdown ---");
        for md in &report.model_breakdown {
            println!(
                "  {}: {} tokens, {} ({} sessions)",
                md.model,
                fmt_tokens(md.total_tokens),
                fmt_cost(md.total_cost),
                md.session_count
            );
        }
        println!();
    }

    if daily && !report.daily_breakdown.is_empty() {
        println!("--- Daily Breakdown ---");
        println!(
            "  {:12} {:>12} {:>12} {:>8}",
            "Date", "Tokens", "Cost", "Sessions"
        );
        for d in &report.daily_breakdown {
            let total_tokens =
                d.total_input + d.total_output + d.total_cache_creation + d.total_cache_read;
            println!(
                "  {:12} {:>12} {:>12} {:>8}",
                d.date,
                fmt_tokens(total_tokens),
                fmt_cost(d.total_cost),
                d.session_count
            );
        }
    }

    Ok(())
}

fn cmd_archive(dry_run: bool, days_threshold: u32) -> Result<()> {
    let projects_dir = claude_token_analyzer::config::resolve_projects_dir()?;
    let archive_dir = claude_token_analyzer::config::resolve_archive_dir()?;

    let expiring = Archiver::find_expiring_sessions(&projects_dir, days_threshold)?;

    if expiring.is_empty() {
        println!(
            "No expiring sessions found (threshold: {} days).",
            days_threshold
        );
        return Ok(());
    }

    println!(
        "Found {} expiring session file(s) (threshold: {} days)",
        expiring.len(),
        days_threshold
    );

    if dry_run {
        println!("\n--- Dry Run (no files will be archived) ---");
        for path in &expiring {
            let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            println!("  {} ({} bytes)", path.display(), size);
        }
        return Ok(());
    }

    let archiver = Archiver::new(&archive_dir);
    let mut manifest = archiver.load_manifest()?;

    let mut archived_count: u64 = 0;
    let mut total_original: u64 = 0;
    let mut total_compressed: u64 = 0;
    let mut failed_count: u64 = 0;

    for path in &expiring {
        match archiver.archive_file(path) {
            Ok(entry) => {
                total_original += entry.original_size;
                total_compressed += entry.compressed_size;
                manifest.entries.push(entry);
                archived_count += 1;
            }
            Err(e) => {
                eprintln!("Failed to archive {}: {}", path.display(), e);
                failed_count += 1;
            }
        }
    }

    manifest.last_updated = Utc::now().to_rfc3339();
    archiver.save_manifest(&manifest)?;

    println!("\n=== Archive Complete ===");
    println!("Archived:   {}", archived_count);
    println!("Failed:     {}", failed_count);
    if total_original > 0 {
        let ratio = total_compressed as f64 / total_original as f64 * 100.0;
        println!(
            "Size:       {} -> {} ({:.1}%)",
            fmt_tokens(total_original),
            fmt_tokens(total_compressed),
            ratio
        );
    }

    Ok(())
}

fn cmd_export(format: &str, output: Option<String>, project: Option<String>) -> Result<()> {
    let db = open_db()?;
    let project_ref = project.as_deref();

    let sessions = db.all_sessions(project_ref)?;

    let content = match format {
        "json" => serde_json::to_string_pretty(&sessions)
            .context("Failed to serialize sessions to JSON")?,
        "csv" => {
            let mut wtr = Vec::new();
            // Write CSV header
            writeln!(
                wtr,
                "session_id,project_path,is_subagent,agent_id,first_timestamp,last_timestamp,total_turns,total_input_tokens,total_output_tokens,total_cache_creation_tokens,total_cache_read_tokens,cache_hit_rate,total_cost_usd"
            )?;
            // Write rows
            for s in &sessions {
                writeln!(
                    wtr,
                    "{},{},{},{},{},{},{},{},{},{},{},{},{}",
                    csv_escape(&s.session_id),
                    csv_escape(&s.project_path),
                    s.is_subagent,
                    s.agent_id.as_deref().unwrap_or(""),
                    s.first_timestamp.as_deref().unwrap_or(""),
                    s.last_timestamp.as_deref().unwrap_or(""),
                    s.total_turns,
                    s.total_input_tokens,
                    s.total_output_tokens,
                    s.total_cache_creation_tokens,
                    s.total_cache_read_tokens,
                    s.cache_hit_rate
                        .map(|v| format!("{:.6}", v))
                        .unwrap_or_default(),
                    s.total_cost_usd
                        .map(|v| format!("{:.6}", v))
                        .unwrap_or_default(),
                )?;
            }
            String::from_utf8(wtr).context("CSV output is not valid UTF-8")?
        }
        other => bail!("Unsupported format: '{}'. Use 'json' or 'csv'.", other),
    };

    match output {
        Some(path) => {
            std::fs::write(&path, &content)
                .with_context(|| format!("Failed to write to {}", path))?;
            println!("Exported {} sessions to {}", sessions.len(), path);
        }
        None => {
            print!("{}", content);
        }
    }

    Ok(())
}

fn cmd_anomalies(threshold: f64, project: Option<String>) -> Result<()> {
    use std::collections::HashMap;

    let db = open_db()?;
    let project_ref = project.as_deref();

    let report = detect_anomalies(&db, threshold, project_ref, 10_000)?;

    println!("=== Claude Token Analyzer: Anomaly Report ===\n");
    println!("Sessions scanned: {}", fmt_count(report.sessions_scanned));
    println!("Stddev threshold: {:.1}x", report.stddev_threshold);
    if let Some(p) = &project {
        println!("Project filter:   {}", short_project_name(p));
    }
    println!();

    if report.anomalies.is_empty() {
        println!("No anomalies detected.");
        return Ok(());
    }

    // Group by type
    let mut by_type: HashMap<String, Vec<&claude_token_analyzer::detector::Anomaly>> =
        HashMap::new();
    for a in &report.anomalies {
        let key = anomaly_type_label(&a.anomaly_type);
        by_type.entry(key.to_string()).or_default().push(a);
    }

    // Summary table
    println!("--- Summary ---");
    println!("  {:<20} {:>8}", "Type", "Count");
    println!("  {}", "─".repeat(30));
    let type_order = [
        "HIGH_COST",
        "HIGH_TOKENS",
        "EXCESSIVE_TOOLS",
        "LOW_CACHE_HIT",
        "COST_INEFFICIENT",
        "UNUSUAL_MODELS",
    ];
    let mut total = 0usize;
    for label in &type_order {
        if let Some(items) = by_type.get(*label) {
            println!("  {:<20} {:>8}", label, fmt_count(items.len() as u64));
            total += items.len();
        }
    }
    println!("  {}", "─".repeat(30));
    println!("  {:<20} {:>8}", "TOTAL", fmt_count(total as u64));
    println!();

    // Detail per type — top 10 each, sorted by stddevs_above descending
    let max_per_type = 10;
    for label in &type_order {
        if let Some(items) = by_type.get(*label) {
            let mut sorted: Vec<_> = items.clone();
            sorted.sort_by(|a, b| {
                b.stddevs_above
                    .partial_cmp(&a.stddevs_above)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let showing = sorted.len().min(max_per_type);
            let remaining = sorted.len().saturating_sub(max_per_type);

            println!(
                "--- {} (top {} of {}) ---",
                label,
                showing,
                sorted.len()
            );
            println!(
                "  {:>3}  {:<24} {:>10} {:>10} {:>8}",
                "#", "Session", "Value", "Threshold", "Stddevs"
            );
            println!("  {}", "─".repeat(60));

            for (i, a) in sorted.iter().take(max_per_type).enumerate() {
                let value_str = format_anomaly_value(&a.anomaly_type, a.value);
                let threshold_str = format_anomaly_value(&a.anomaly_type, a.threshold);
                println!(
                    "  {:>3}  {:<24} {:>10} {:>10} {:>7.1}x",
                    i + 1,
                    truncate_session_id(&a.session_id, 24),
                    value_str,
                    threshold_str,
                    a.stddevs_above
                );
            }

            if remaining > 0 {
                println!("  ... and {} more", remaining);
            }
            println!();
        }
    }

    Ok(())
}

fn anomaly_type_label(t: &claude_token_analyzer::detector::AnomalyType) -> &'static str {
    use claude_token_analyzer::detector::AnomalyType;
    match t {
        AnomalyType::HighTokenUsage => "HIGH_TOKENS",
        AnomalyType::HighCost => "HIGH_COST",
        AnomalyType::UnusualModelMix => "UNUSUAL_MODELS",
        AnomalyType::ExcessiveToolUse => "EXCESSIVE_TOOLS",
        AnomalyType::LowCacheHitRate => "LOW_CACHE_HIT",
        AnomalyType::CostInefficient => "COST_INEFFICIENT",
    }
}

fn format_anomaly_value(t: &claude_token_analyzer::detector::AnomalyType, v: f64) -> String {
    use claude_token_analyzer::detector::AnomalyType;
    match t {
        AnomalyType::HighCost => fmt_cost(v),
        AnomalyType::HighTokenUsage => fmt_tokens(v as u64),
        AnomalyType::ExcessiveToolUse => fmt_count(v as u64),
        AnomalyType::LowCacheHitRate => fmt_pct(v),
        AnomalyType::UnusualModelMix => format!("{}", v as u32),
        AnomalyType::CostInefficient => fmt_cost(v),
    }
}

fn cmd_trend(granularity: &str, days: u32, project: Option<String>) -> Result<()> {
    let db = open_db()?;
    let project_ref = project.as_deref();

    // Validate granularity
    match granularity {
        "daily" | "weekly" | "monthly" => {}
        other => bail!(
            "Unsupported granularity: '{}'. Use 'daily', 'weekly', or 'monthly'.",
            other
        ),
    }

    let trend = analyze_trend(&db, project_ref, days)?;

    println!("=== Claude Token Analyzer: Trend Report ===\n");
    println!("Granularity: {}", granularity);
    println!(
        "Period: last {} days ({} data points)",
        days, trend.total_days
    );
    if let Some(p) = &project {
        println!("Project: {}", p);
    }
    println!("Avg daily cost:   {}", fmt_cost(trend.avg_daily_cost));
    println!("Avg daily tokens: {}", fmt_tokens(trend.avg_daily_tokens));
    if let Some(peak) = &trend.peak_day {
        println!(
            "Peak day: {} ({}, {} sessions)",
            peak.date,
            fmt_cost(peak.total_cost),
            peak.session_count
        );
    }
    println!();

    if !trend.data_points.is_empty() {
        println!(
            "  {:12} {:>12} {:>12} {:>8}",
            "Date", "Tokens", "Cost", "Sessions"
        );
        println!("  {}", "-".repeat(48));

        // For weekly/monthly, aggregate; for daily, show as-is
        match granularity {
            "daily" => {
                for d in &trend.data_points {
                    let total_tokens = d.total_input
                        + d.total_output
                        + d.total_cache_creation
                        + d.total_cache_read;
                    println!(
                        "  {:12} {:>12} {:>12} {:>8}",
                        d.date,
                        fmt_tokens(total_tokens),
                        fmt_cost(d.total_cost),
                        d.session_count
                    );
                }
            }
            "weekly" => {
                print_aggregated_trend(&trend.data_points, 7);
            }
            "monthly" => {
                print_aggregated_trend(&trend.data_points, 30);
            }
            _ => unreachable!(),
        }
    }

    Ok(())
}

/// Aggregate daily data points into buckets of `bucket_days` size
fn print_aggregated_trend(
    data_points: &[claude_token_analyzer::storage::DailyStats],
    bucket_days: usize,
) {
    let chunks: Vec<&[claude_token_analyzer::storage::DailyStats]> =
        data_points.chunks(bucket_days).collect();
    for chunk in chunks {
        if chunk.is_empty() {
            continue;
        }
        let start_date = &chunk[0].date;
        let end_date = &chunk[chunk.len() - 1].date;
        let total_tokens: u64 = chunk
            .iter()
            .map(|d| d.total_input + d.total_output + d.total_cache_creation + d.total_cache_read)
            .sum();
        let total_cost: f64 = chunk.iter().map(|d| d.total_cost).sum();
        let total_sessions: u64 = chunk.iter().map(|d| d.session_count).sum();

        let label = if start_date == end_date {
            start_date.clone()
        } else {
            format!("{}~{}", &start_date[5..], &end_date[5..])
        };

        println!(
            "  {:12} {:>12} {:>12} {:>8}",
            label,
            fmt_tokens(total_tokens),
            fmt_cost(total_cost),
            total_sessions
        );
    }
}

/// Find a JSONL file by session ID, returning an error if not found.
fn find_session_file(projects_dir: &Path, session_id: &str) -> Result<PathBuf> {
    claude_token_analyzer::session_finder::find_session_file(projects_dir, session_id).ok_or_else(
        || {
            anyhow::anyhow!(
                "Session file not found: {} (searched under {})",
                session_id,
                projects_dir.display()
            )
        },
    )
}

// === Main ===

fn main() -> Result<()> {
    // Init tracing to stderr so warn!() messages from sync_all are visible
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Sync { verbose } => cmd_sync(verbose),
        Commands::Analyze {
            session,
            project,
            global,
        } => {
            if let Some(session_id) = session {
                cmd_analyze_session(&session_id)
            } else if let Some(project_path) = project {
                cmd_analyze_project(&project_path)
            } else if global {
                cmd_analyze_global()
            } else {
                bail!("Specify one of: --session <ID>, --project <PATH>, or --global");
            }
        }
        Commands::Cost {
            month,
            daily,
            project,
        } => cmd_cost(month, daily, project),
        Commands::Archive {
            dry_run,
            days_threshold,
        } => cmd_archive(dry_run, days_threshold),
        Commands::Export {
            format,
            output,
            project,
        } => cmd_export(&format, output, project),
        Commands::Anomalies { threshold, project } => cmd_anomalies(threshold, project),
        Commands::Trend {
            granularity,
            days,
            project,
        } => cmd_trend(&granularity, days, project),
    }
}
