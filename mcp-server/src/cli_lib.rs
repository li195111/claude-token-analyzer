//! Library-accessible CLI command logic — extracted from bin/cli.rs for testability.
//!
//! All commands accept parameters instead of resolving paths/DB internally,
//! enabling dependency injection in unit tests.

use std::path::Path;

use anyhow::{bail, Context, Result};
use chrono::Utc;

use crate::analyzer::{analyze_cost, analyze_trend};
use crate::detector::detect_anomalies;
use crate::format::{fmt_cost, fmt_count, fmt_tokens};
use crate::pricing::PricingTable;
use crate::storage::Database;

// ============================================================
// Validation helpers
// ============================================================

/// Validate that a month string is in YYYY-MM format with a valid month number.
fn validate_month_format(month: &str) -> Result<()> {
    if month.len() != 7
        || !month[..4].chars().all(|c| c.is_ascii_digit())
        || month.chars().nth(4) != Some('-')
        || !month[5..].chars().all(|c| c.is_ascii_digit())
    {
        bail!(
            "Invalid month format '{}'. Expected YYYY-MM (e.g., 2026-04).",
            month
        );
    }
    let mm: u32 = month[5..].parse().context("Invalid month number")?;
    if !(1..=12).contains(&mm) {
        bail!("Month value out of range: {}. Expected 01-12.", &month[5..]);
    }
    Ok(())
}

// ============================================================
// Public command functions
// ============================================================

/// Sync sessions from a projects directory into the database.
///
/// Creates the database file at `db_path` if it does not already exist.
///
/// # Parameters
/// - `db_path` — path to the SQLite database (created if absent)
/// - `projects_dir` — directory containing Claude Code session JSONL files
/// - `verbose` — print per-file progress to stdout
pub fn cmd_sync(db_path: &Path, projects_dir: &Path, verbose: bool) -> Result<()> {
    let db = Database::open(db_path)?;
    let pricing = PricingTable::from_env_or_embedded()?;

    if verbose {
        println!("Syncing from: {}", projects_dir.display());
        println!("Database:     {}", db_path.display());
    }

    let report = db.sync_all(projects_dir, &pricing, verbose)?;

    println!("=== Sync Complete ===");
    println!(
        "Total JSONL files: {}",
        fmt_count(report.total_files_on_disk)
    );
    println!("New/modified:      {}", fmt_count(report.files_to_sync));
    println!("Synced:            {}", fmt_count(report.files_synced));
    println!("Failed:            {}", fmt_count(report.files_failed));
    println!("Sessions upserted: {}", fmt_count(report.sessions_upserted));

    Ok(())
}

/// Print a cost report for the given month.
///
/// # Parameters
/// - `db` — open database connection
/// - `month` — filter in `YYYY-MM` format; uses current month when `None`
/// - `daily` — include a daily cost breakdown in the output
/// - `project` — optional project path filter
pub fn cmd_cost(
    db: &Database,
    month: Option<String>,
    daily: bool,
    project: Option<String>,
) -> Result<()> {
    let month_str = match month {
        Some(ref m) => {
            validate_month_format(m)?;
            m.clone()
        }
        None => Utc::now().format("%Y-%m").to_string(),
    };
    let project_ref = project.as_deref();

    let report = analyze_cost(db, &month_str, project_ref)?;

    println!("=== Claude Token Analyzer: Cost Report ===\n");
    println!("Month: {}", report.month);
    if let Some(ref p) = project {
        println!("Project: {}", p);
    }
    println!("Total cost: {}", fmt_cost(report.total_cost));

    if daily && !report.daily_breakdown.is_empty() {
        println!();
        println!("--- Daily Breakdown ---");
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

/// Print a trend report.
///
/// # Parameters
/// - `db` — open database connection
/// - `granularity` — one of `"daily"`, `"weekly"`, or `"monthly"`
/// - `days` — number of past days to include in the report
/// - `project` — optional project path filter
pub fn cmd_trend(
    db: &Database,
    granularity: &str,
    days: u32,
    project: Option<String>,
) -> Result<()> {
    match granularity {
        "daily" | "weekly" | "monthly" => {}
        other => bail!(
            "Unsupported granularity: '{}'. Use 'daily', 'weekly', or 'monthly'.",
            other
        ),
    }
    let project_ref = project.as_deref();

    let trend = analyze_trend(db, project_ref, days)?;

    println!("=== Claude Token Analyzer: Trend Report ===\n");
    println!("Granularity: {}", granularity);
    println!(
        "Period: last {} days ({} data points)",
        days, trend.total_days
    );
    if let Some(ref p) = project {
        println!("Project: {}", p);
    }
    println!("Avg daily cost:   {}", fmt_cost(trend.avg_daily_cost));
    println!("Avg daily tokens: {}", fmt_tokens(trend.avg_daily_tokens));

    Ok(())
}

/// Print an anomaly detection report.
///
/// # Parameters
/// - `db` — open database connection
/// - `threshold` — standard deviation multiplier for anomaly flagging (must be ≥ 0)
/// - `project` — optional project path filter
pub fn cmd_anomalies(db: &Database, threshold: f64, project: Option<String>) -> Result<()> {
    if threshold < 0.0 {
        bail!("Threshold must be non-negative, got {}.", threshold);
    }
    let project_ref = project.as_deref();

    let report = detect_anomalies(db, threshold, project_ref, 10_000)?;

    println!("=== Claude Token Analyzer: Anomaly Report ===\n");
    println!("Sessions scanned: {}", fmt_count(report.sessions_scanned));
    println!("Stddev threshold: {:.1}x", report.stddev_threshold);

    if report.anomalies.is_empty() {
        println!("No anomalies detected.");
    } else {
        println!("Anomalies found: {}", report.anomalies.len());
    }

    Ok(())
}
