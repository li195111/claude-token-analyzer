//! CLI logic unit tests — TDD Phase 2 (RED)
//!
//! 這些測試參照尚未實作的 `cli_lib` 模組（Phase 3 重構後從 bin/cli.rs 移出）。
//! 目前應全部編譯失敗（CompileError），待 Phase 3 完成後轉為綠燈。
//!
//! 設計原則：每個 test 只測一個 CLI 子命令的核心邏輯，
//! 不測 stdout 格式（格式屬 presentation 層，不屬 logic 層）。

use std::path::PathBuf;
use tempfile::TempDir;

use claude_token_analyzer::cli_lib::{cmd_anomalies, cmd_cost, cmd_sync, cmd_trend};
use claude_token_analyzer::storage::Database;

// ============================================================
// Helper
// ============================================================

fn temp_db() -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    (dir, db_path)
}

fn empty_projects_dir() -> TempDir {
    TempDir::new().unwrap()
}

// ============================================================
// cmd_sync
// ============================================================

#[test]
fn test_cmd_sync_empty_projects_dir_returns_ok() {
    let (dir, db_path) = temp_db();
    let projects_dir = empty_projects_dir();

    let result = cmd_sync(db_path.as_path(), projects_dir.path(), false);
    assert!(result.is_ok(), "sync with empty dir should succeed");
    // After sync, DB file should exist
    assert!(db_path.exists(), "DB file should be created after sync");
    drop(dir);
}

#[test]
fn test_cmd_sync_verbose_does_not_panic() {
    let (dir, db_path) = temp_db();
    let projects_dir = empty_projects_dir();

    let result = cmd_sync(db_path.as_path(), projects_dir.path(), true);
    assert!(result.is_ok());
    drop(dir);
}

// ============================================================
// cmd_cost
// ============================================================

#[test]
fn test_cmd_cost_current_month_on_empty_db_returns_ok() {
    let (dir, db_path) = temp_db();
    let db = Database::open(&db_path).unwrap();

    let result = cmd_cost(&db, None, false, None);
    assert!(
        result.is_ok(),
        "cost on empty DB should return Ok (zero cost)"
    );
    drop(dir);
}

#[test]
fn test_cmd_cost_specific_month_invalid_format_returns_err() {
    let (dir, db_path) = temp_db();
    let db = Database::open(&db_path).unwrap();

    let result = cmd_cost(&db, Some("2026/04".to_string()), false, None);
    assert!(
        result.is_err(),
        "month format 'YYYY/MM' should be rejected (must be YYYY-MM)"
    );
    drop(dir);
}

#[test]
fn test_cmd_cost_daily_flag_on_empty_db_returns_ok() {
    let (dir, db_path) = temp_db();
    let db = Database::open(&db_path).unwrap();

    let result = cmd_cost(&db, None, true, None);
    assert!(result.is_ok());
    drop(dir);
}

// ============================================================
// cmd_trend
// ============================================================

#[test]
fn test_cmd_trend_daily_granularity_returns_ok() {
    let (dir, db_path) = temp_db();
    let db = Database::open(&db_path).unwrap();

    let result = cmd_trend(&db, "daily", 30, None);
    assert!(result.is_ok());
    drop(dir);
}

#[test]
fn test_cmd_trend_weekly_granularity_returns_ok() {
    let (dir, db_path) = temp_db();
    let db = Database::open(&db_path).unwrap();

    let result = cmd_trend(&db, "weekly", 90, None);
    assert!(result.is_ok());
    drop(dir);
}

#[test]
fn test_cmd_trend_invalid_granularity_returns_err() {
    let (dir, db_path) = temp_db();
    let db = Database::open(&db_path).unwrap();

    let result = cmd_trend(&db, "hourly", 7, None);
    assert!(result.is_err(), "'hourly' is not a valid granularity");
    drop(dir);
}

// ============================================================
// cmd_anomalies
// ============================================================

#[test]
fn test_cmd_anomalies_default_threshold_returns_ok() {
    let (dir, db_path) = temp_db();
    let db = Database::open(&db_path).unwrap();

    let result = cmd_anomalies(&db, 2.0, None);
    assert!(
        result.is_ok(),
        "anomalies on empty DB should return Ok (no anomalies)"
    );
    drop(dir);
}

#[test]
fn test_cmd_anomalies_negative_threshold_returns_err() {
    let (dir, db_path) = temp_db();
    let db = Database::open(&db_path).unwrap();

    let result = cmd_anomalies(&db, -1.0, None);
    assert!(result.is_err(), "negative threshold should be rejected");
    drop(dir);
}
