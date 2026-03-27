use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::parser::parse_jsonl_file;
use crate::pricing::PricingTable;
use crate::types::{ParseResult, TokenUsage, ToolUsageStat};

// === Query result types ===

/// A row from the sessions table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRow {
    pub session_id: String,
    pub project_path: String,
    pub is_subagent: bool,
    pub agent_id: Option<String>,
    pub first_timestamp: Option<String>,
    pub last_timestamp: Option<String>,
    pub total_turns: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_creation_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub cache_hit_rate: Option<f64>,
    pub total_cost_usd: Option<f64>,
}

/// Global aggregate statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalStats {
    pub total_sessions: u64,
    pub total_projects: u64,
    pub total_tokens: u64,
    pub total_cost_usd: f64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_creation_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub avg_cache_hit_rate: f64,
}

/// Per-project aggregate statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectStats {
    pub project_path: String,
    pub session_count: u64,
    pub total_tokens: u64,
    pub total_cost_usd: f64,
    pub avg_cache_hit_rate: f64,
}

/// Project listing summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub project_path: String,
    pub session_count: u64,
    pub total_cost_usd: f64,
}

/// Daily token usage trend data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyStats {
    pub date: String,
    pub total_input: u64,
    pub total_output: u64,
    pub total_cache_creation: u64,
    pub total_cache_read: u64,
    pub total_cost: f64,
    pub session_count: u64,
}

/// Report from a full sync operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncReport {
    pub total_files_on_disk: u64,
    pub files_to_sync: u64,
    pub files_synced: u64,
    pub files_failed: u64,
    pub sessions_upserted: u64,
}

// === Database ===

pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open or create a database at the given path
    pub fn open(path: &Path) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create database directory: {}", parent.display())
            })?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open database: {}", path.display()))?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// Open an in-memory database (for testing)
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("Failed to open in-memory database")?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// Initialize the database schema
    fn init_schema(&self) -> Result<()> {
        self.conn
            .execute_batch(
                "
            CREATE TABLE IF NOT EXISTS sessions (
                session_id TEXT PRIMARY KEY,
                project_path TEXT NOT NULL,
                is_subagent INTEGER NOT NULL DEFAULT 0,
                agent_id TEXT,
                first_timestamp TEXT,
                last_timestamp TEXT,
                total_turns INTEGER NOT NULL DEFAULT 0,
                total_input_tokens INTEGER NOT NULL DEFAULT 0,
                total_output_tokens INTEGER NOT NULL DEFAULT 0,
                total_cache_creation_tokens INTEGER NOT NULL DEFAULT 0,
                total_cache_read_tokens INTEGER NOT NULL DEFAULT 0,
                cache_hit_rate REAL,
                total_cost_usd REAL,
                failed_lines INTEGER NOT NULL DEFAULT 0,
                total_lines INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS session_models (
                session_id TEXT NOT NULL REFERENCES sessions(session_id),
                model TEXT NOT NULL,
                turn_count INTEGER NOT NULL DEFAULT 0,
                total_input INTEGER NOT NULL DEFAULT 0,
                total_output INTEGER NOT NULL DEFAULT 0,
                total_cache_creation INTEGER NOT NULL DEFAULT 0,
                total_cache_read INTEGER NOT NULL DEFAULT 0,
                cost_usd REAL,
                PRIMARY KEY (session_id, model)
            );

            CREATE TABLE IF NOT EXISTS session_tools (
                session_id TEXT NOT NULL REFERENCES sessions(session_id),
                tool_name TEXT NOT NULL,
                invocation_count INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (session_id, tool_name)
            );

            CREATE TABLE IF NOT EXISTS compression_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL REFERENCES sessions(session_id),
                turn_index INTEGER NOT NULL,
                timestamp TEXT NOT NULL,
                cache_read_before INTEGER NOT NULL,
                cache_read_after INTEGER NOT NULL,
                drop_percentage REAL NOT NULL
            );

            CREATE TABLE IF NOT EXISTS turn_durations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL REFERENCES sessions(session_id),
                duration_ms INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS sync_state (
                file_path TEXT PRIMARY KEY,
                mtime_secs INTEGER NOT NULL,
                last_synced TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_project ON sessions(project_path);
            CREATE INDEX IF NOT EXISTS idx_sessions_timestamp ON sessions(first_timestamp);
            CREATE INDEX IF NOT EXISTS idx_sessions_cost ON sessions(total_cost_usd DESC);
            CREATE INDEX IF NOT EXISTS idx_sessions_subagent ON sessions(is_subagent);
            CREATE INDEX IF NOT EXISTS idx_session_models_model ON session_models(model);
            CREATE INDEX IF NOT EXISTS idx_session_tools_name ON session_tools(tool_name);
            ",
            )
            .context("Failed to initialize database schema")?;
        Ok(())
    }

    /// Upsert a parsed session result into the database
    pub fn upsert_session(&self, result: &ParseResult, pricing: &PricingTable) -> Result<()> {
        // Calculate per-model costs and total cost
        let mut total_cost: f64 = 0.0;
        let model_costs: Vec<(&crate::types::ModelUsage, f64)> = result
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
                total_cost += cost.total_cost;
                (mu, cost.total_cost)
            })
            .collect();

        // Calculate cache hit rate
        let denominator = result.total_input_tokens
            + result.total_cache_read_tokens
            + result.total_cache_creation_tokens;
        let cache_hit_rate = if denominator > 0 {
            Some(result.total_cache_read_tokens as f64 / denominator as f64)
        } else {
            None
        };

        let first_ts = result.first_timestamp.map(|t| t.to_rfc3339());
        let last_ts = result.last_timestamp.map(|t| t.to_rfc3339());

        let tx = self
            .conn
            .unchecked_transaction()
            .context("Failed to begin transaction")?;

        // Upsert session
        tx.execute(
            "INSERT OR REPLACE INTO sessions (
                session_id, project_path, is_subagent, agent_id,
                first_timestamp, last_timestamp,
                total_turns, total_input_tokens, total_output_tokens,
                total_cache_creation_tokens, total_cache_read_tokens,
                cache_hit_rate, total_cost_usd, failed_lines, total_lines
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                result.session_id,
                result.project_path,
                result.is_subagent as i32,
                result.agent_id,
                first_ts,
                last_ts,
                result.total_turns as i64,
                result.total_input_tokens as i64,
                result.total_output_tokens as i64,
                result.total_cache_creation_tokens as i64,
                result.total_cache_read_tokens as i64,
                cache_hit_rate,
                total_cost,
                result.failed_lines as i64,
                result.total_lines as i64,
            ],
        )?;

        // Delete existing child rows
        tx.execute(
            "DELETE FROM session_models WHERE session_id = ?1",
            params![result.session_id],
        )?;
        tx.execute(
            "DELETE FROM session_tools WHERE session_id = ?1",
            params![result.session_id],
        )?;
        tx.execute(
            "DELETE FROM compression_events WHERE session_id = ?1",
            params![result.session_id],
        )?;
        tx.execute(
            "DELETE FROM turn_durations WHERE session_id = ?1",
            params![result.session_id],
        )?;

        // Insert model usage rows
        for (mu, cost) in &model_costs {
            tx.execute(
                "INSERT INTO session_models (
                    session_id, model, turn_count,
                    total_input, total_output, total_cache_creation, total_cache_read,
                    cost_usd
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    result.session_id,
                    mu.model,
                    mu.turn_count as i64,
                    mu.total_input as i64,
                    mu.total_output as i64,
                    mu.total_cache_creation as i64,
                    mu.total_cache_read as i64,
                    cost,
                ],
            )?;
        }

        // Insert tool usage rows
        for tu in &result.tool_usage {
            tx.execute(
                "INSERT INTO session_tools (session_id, tool_name, invocation_count)
                 VALUES (?1, ?2, ?3)",
                params![result.session_id, tu.name, tu.invocation_count as i64,],
            )?;
        }

        // Insert compression events
        for ce in &result.compression_events {
            tx.execute(
                "INSERT INTO compression_events (
                    session_id, turn_index, timestamp,
                    cache_read_before, cache_read_after, drop_percentage
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    result.session_id,
                    ce.turn_index as i64,
                    ce.timestamp.to_rfc3339(),
                    ce.cache_read_before as i64,
                    ce.cache_read_after as i64,
                    ce.drop_percentage,
                ],
            )?;
        }

        // Insert turn durations
        for &dur in &result.turn_durations_ms {
            tx.execute(
                "INSERT INTO turn_durations (session_id, duration_ms)
                 VALUES (?1, ?2)",
                params![result.session_id, dur as i64],
            )?;
        }

        tx.commit().context("Failed to commit session upsert")?;
        Ok(())
    }

    /// Query all sessions for a given project path
    pub fn sessions_by_project(&self, project_path: &str) -> Result<Vec<SessionRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT session_id, project_path, is_subagent, agent_id,
                    first_timestamp, last_timestamp,
                    total_turns, total_input_tokens, total_output_tokens,
                    total_cache_creation_tokens, total_cache_read_tokens,
                    cache_hit_rate, total_cost_usd
             FROM sessions
             WHERE project_path = ?1
             ORDER BY first_timestamp DESC",
        )?;

        let rows = stmt.query_map(params![project_path], row_to_session_row)?;
        rows.collect::<Result<Vec<_>, _>>()
            .context("Failed to query sessions by project")
    }

    /// Query global aggregate statistics
    pub fn global_stats(&self) -> Result<GlobalStats> {
        let mut stmt = self.conn.prepare(
            "SELECT
                COUNT(*) as total_sessions,
                COUNT(DISTINCT project_path) as total_projects,
                COALESCE(SUM(total_input_tokens + total_output_tokens + total_cache_creation_tokens + total_cache_read_tokens), 0) as total_tokens,
                COALESCE(SUM(total_cost_usd), 0.0) as total_cost_usd,
                COALESCE(SUM(total_input_tokens), 0) as total_input_tokens,
                COALESCE(SUM(total_output_tokens), 0) as total_output_tokens,
                COALESCE(SUM(total_cache_creation_tokens), 0) as total_cache_creation_tokens,
                COALESCE(SUM(total_cache_read_tokens), 0) as total_cache_read_tokens,
                COALESCE(AVG(cache_hit_rate), 0.0) as avg_cache_hit_rate
             FROM sessions"
        )?;

        stmt.query_row([], |row| {
            Ok(GlobalStats {
                total_sessions: row.get::<_, i64>(0)? as u64,
                total_projects: row.get::<_, i64>(1)? as u64,
                total_tokens: row.get::<_, i64>(2)? as u64,
                total_cost_usd: row.get(3)?,
                total_input_tokens: row.get::<_, i64>(4)? as u64,
                total_output_tokens: row.get::<_, i64>(5)? as u64,
                total_cache_creation_tokens: row.get::<_, i64>(6)? as u64,
                total_cache_read_tokens: row.get::<_, i64>(7)? as u64,
                avg_cache_hit_rate: row.get(8)?,
            })
        })
        .context("Failed to query global stats")
    }

    /// Query per-project aggregate statistics
    pub fn project_stats(&self, project_path: &str) -> Result<ProjectStats> {
        let mut stmt = self.conn.prepare(
            "SELECT
                project_path,
                COUNT(*) as session_count,
                COALESCE(SUM(total_input_tokens + total_output_tokens + total_cache_creation_tokens + total_cache_read_tokens), 0) as total_tokens,
                COALESCE(SUM(total_cost_usd), 0.0) as total_cost_usd,
                COALESCE(AVG(cache_hit_rate), 0.0) as avg_cache_hit_rate
             FROM sessions
             WHERE project_path = ?1
             GROUP BY project_path"
        )?;

        stmt.query_row(params![project_path], |row| {
            Ok(ProjectStats {
                project_path: row.get(0)?,
                session_count: row.get::<_, i64>(1)? as u64,
                total_tokens: row.get::<_, i64>(2)? as u64,
                total_cost_usd: row.get(3)?,
                avg_cache_hit_rate: row.get(4)?,
            })
        })
        .context("Failed to query project stats")
    }

    /// List all unique projects with session counts and total costs
    pub fn list_projects(&self) -> Result<Vec<ProjectSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT
                project_path,
                COUNT(*) as session_count,
                COALESCE(SUM(total_cost_usd), 0.0) as total_cost_usd
             FROM sessions
             GROUP BY project_path
             ORDER BY total_cost_usd DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ProjectSummary {
                project_path: row.get(0)?,
                session_count: row.get::<_, i64>(1)? as u64,
                total_cost_usd: row.get(2)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .context("Failed to list projects")
    }

    /// Query sessions sorted by cost descending, limited to top N
    pub fn top_sessions_by_cost(&self, limit: u32) -> Result<Vec<SessionRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT session_id, project_path, is_subagent, agent_id,
                    first_timestamp, last_timestamp,
                    total_turns, total_input_tokens, total_output_tokens,
                    total_cache_creation_tokens, total_cache_read_tokens,
                    cache_hit_rate, total_cost_usd
             FROM sessions
             ORDER BY total_cost_usd DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit], row_to_session_row)?;
        rows.collect::<Result<Vec<_>, _>>()
            .context("Failed to query top sessions by cost")
    }

    /// Query tool usage ranking across all sessions
    pub fn global_tool_ranking(&self) -> Result<Vec<ToolUsageStat>> {
        let mut stmt = self.conn.prepare(
            "SELECT tool_name, SUM(invocation_count) as total_invocations
             FROM session_tools
             GROUP BY tool_name
             ORDER BY total_invocations DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ToolUsageStat {
                name: row.get(0)?,
                invocation_count: row.get::<_, i64>(1)? as u64,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .context("Failed to query global tool ranking")
    }

    /// Query daily token usage trend, optionally filtered by project
    pub fn daily_trend(
        &self,
        project_path: Option<&str>,
        last_n_days: u32,
    ) -> Result<Vec<DailyStats>> {
        let query = match project_path {
            Some(_) => {
                "SELECT
                    DATE(first_timestamp) as date,
                    COALESCE(SUM(total_input_tokens), 0) as total_input,
                    COALESCE(SUM(total_output_tokens), 0) as total_output,
                    COALESCE(SUM(total_cache_creation_tokens), 0) as total_cache_creation,
                    COALESCE(SUM(total_cache_read_tokens), 0) as total_cache_read,
                    COALESCE(SUM(total_cost_usd), 0.0) as total_cost,
                    COUNT(*) as session_count
                 FROM sessions
                 WHERE project_path = ?1
                   AND first_timestamp IS NOT NULL
                   AND DATE(first_timestamp) >= DATE('now', '-' || ?2 || ' days')
                 GROUP BY DATE(first_timestamp)
                 ORDER BY date ASC"
            }
            None => {
                "SELECT
                    DATE(first_timestamp) as date,
                    COALESCE(SUM(total_input_tokens), 0) as total_input,
                    COALESCE(SUM(total_output_tokens), 0) as total_output,
                    COALESCE(SUM(total_cache_creation_tokens), 0) as total_cache_creation,
                    COALESCE(SUM(total_cache_read_tokens), 0) as total_cache_read,
                    COALESCE(SUM(total_cost_usd), 0.0) as total_cost,
                    COUNT(*) as session_count
                 FROM sessions
                 WHERE first_timestamp IS NOT NULL
                   AND DATE(first_timestamp) >= DATE('now', '-' || ?2 || ' days')
                 GROUP BY DATE(first_timestamp)
                 ORDER BY date ASC"
            }
        };

        let mut stmt = self.conn.prepare(query)?;

        let rows = match project_path {
            Some(pp) => stmt.query_map(params![pp, last_n_days], row_to_daily_stats)?,
            None => stmt.query_map(
                params![rusqlite::types::Null, last_n_days],
                row_to_daily_stats,
            )?,
        };

        rows.collect::<Result<Vec<_>, _>>()
            .context("Failed to query daily trend")
    }

    /// Find JSONL files that need syncing (new or modified since last sync)
    pub fn find_files_to_sync(&self, base_dir: &Path) -> Result<Vec<PathBuf>> {
        let jsonl_files = walk_jsonl_files(base_dir)?;
        self.find_files_to_sync_from(jsonl_files)
    }

    /// Filter pre-walked files to only those needing sync
    fn find_files_to_sync_from(&self, jsonl_files: Vec<PathBuf>) -> Result<Vec<PathBuf>> {
        let mut files_to_sync = Vec::new();

        for file_path in jsonl_files {
            let mtime = file_mtime(&file_path)?;
            let path_str = file_path.to_string_lossy();

            let needs_sync = match self.conn.query_row(
                "SELECT mtime_secs FROM sync_state WHERE file_path = ?1",
                params![path_str.as_ref()],
                |row| row.get::<_, i64>(0),
            ) {
                Ok(stored_mtime) => mtime > stored_mtime,
                Err(rusqlite::Error::QueryReturnedNoRows) => true,
                Err(e) => return Err(e).context("Failed to query sync state"),
            };

            if needs_sync {
                files_to_sync.push(file_path);
            }
        }

        Ok(files_to_sync)
    }

    /// Update sync state after processing a file
    pub fn update_sync_state(&self, file_path: &Path, mtime: i64) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let path_str = file_path.to_string_lossy();

        self.conn.execute(
            "INSERT OR REPLACE INTO sync_state (file_path, mtime_secs, last_synced)
             VALUES (?1, ?2, ?3)",
            params![path_str.as_ref(), mtime, now],
        )?;

        Ok(())
    }

    /// Full sync: parse and upsert all new/modified files
    pub fn sync_all(
        &self,
        base_dir: &Path,
        pricing: &PricingTable,
        verbose: bool,
    ) -> Result<SyncReport> {
        let all_files = walk_jsonl_files(base_dir)?;
        let total_files_on_disk = all_files.len() as u64;
        let files = self.find_files_to_sync_from(all_files)?;
        let files_to_sync = files.len() as u64;

        let mut files_synced: u64 = 0;
        let mut files_failed: u64 = 0;
        let mut sessions_upserted: u64 = 0;

        // Process files one at a time — each upsert_session manages its own transaction
        for file_path in &files {
            match self.process_single_file(file_path, pricing, verbose) {
                Ok(()) => {
                    files_synced += 1;
                    sessions_upserted += 1;
                }
                Err(e) => {
                    files_failed += 1;
                    warn!("Failed to process {}: {:#}", file_path.display(), e);
                }
            }
        }

        if verbose {
            info!(
                "Sync complete: total={}, to_sync={}, synced={}, failed={}, upserted={}",
                total_files_on_disk, files_to_sync, files_synced, files_failed, sessions_upserted
            );
        }

        Ok(SyncReport {
            total_files_on_disk,
            files_to_sync,
            files_synced,
            files_failed,
            sessions_upserted,
        })
    }

    /// Process a single JSONL file: parse, upsert, update sync state
    fn process_single_file(
        &self,
        file_path: &Path,
        pricing: &PricingTable,
        verbose: bool,
    ) -> Result<()> {
        if verbose {
            info!("Processing: {}", file_path.display());
        }

        let result = parse_jsonl_file(file_path)
            .with_context(|| format!("Failed to parse {}", file_path.display()))?;

        self.upsert_session(&result, pricing)?;

        let mtime = file_mtime(file_path)?;
        self.update_sync_state(file_path, mtime)?;

        Ok(())
    }

    /// Query tool usage ranking for a specific project's sessions
    /// Returns Vec<(tool_name, total_invocations, session_count)>
    pub fn project_tool_ranking(
        &self,
        project_path: &str,
    ) -> Result<Vec<(String, u64, u64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT st.tool_name,
                    SUM(st.invocation_count) as total_invocations,
                    COUNT(DISTINCT st.session_id) as session_count
             FROM session_tools st
             JOIN sessions s ON st.session_id = s.session_id
             WHERE s.project_path = ?1
             GROUP BY st.tool_name
             ORDER BY total_invocations DESC",
        )?;

        let rows = stmt.query_map(params![project_path], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)? as u64,
                row.get::<_, i64>(2)? as u64,
            ))
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .context("Failed to query project tool ranking")
    }

    /// Query model distribution for a specific project's sessions
    /// Returns Vec<(model, total_tokens, total_cost, session_count)>
    pub fn project_model_distribution(
        &self,
        project_path: &str,
    ) -> Result<Vec<(String, u64, f64, u64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT sm.model,
                    SUM(sm.total_input + sm.total_output + sm.total_cache_creation + sm.total_cache_read) as total_tokens,
                    COALESCE(SUM(sm.cost_usd), 0.0) as total_cost,
                    COUNT(DISTINCT sm.session_id) as session_count
             FROM session_models sm
             JOIN sessions s ON sm.session_id = s.session_id
             WHERE s.project_path = ?1
             GROUP BY sm.model
             ORDER BY total_tokens DESC",
        )?;

        let rows = stmt.query_map(params![project_path], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)? as u64,
                row.get::<_, f64>(2)?,
                row.get::<_, i64>(3)? as u64,
            ))
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .context("Failed to query project model distribution")
    }

    /// Query subagent ratio across all sessions (or filtered by project)
    /// Returns (main_sessions, subagent_sessions, main_tokens, subagent_tokens)
    pub fn subagent_ratio(
        &self,
        project_path: Option<&str>,
    ) -> Result<(u64, u64, u64, u64)> {
        let (query_main, query_sub) = match project_path {
            Some(_) => (
                "SELECT COUNT(*),
                        COALESCE(SUM(total_input_tokens + total_output_tokens + total_cache_creation_tokens + total_cache_read_tokens), 0)
                 FROM sessions WHERE is_subagent = 0 AND project_path = ?1",
                "SELECT COUNT(*),
                        COALESCE(SUM(total_input_tokens + total_output_tokens + total_cache_creation_tokens + total_cache_read_tokens), 0)
                 FROM sessions WHERE is_subagent = 1 AND project_path = ?1",
            ),
            None => (
                "SELECT COUNT(*),
                        COALESCE(SUM(total_input_tokens + total_output_tokens + total_cache_creation_tokens + total_cache_read_tokens), 0)
                 FROM sessions WHERE is_subagent = 0",
                "SELECT COUNT(*),
                        COALESCE(SUM(total_input_tokens + total_output_tokens + total_cache_creation_tokens + total_cache_read_tokens), 0)
                 FROM sessions WHERE is_subagent = 1",
            ),
        };

        let (main_sessions, main_tokens) = match project_path {
            Some(pp) => self.conn.query_row(query_main, params![pp], |row| {
                Ok((row.get::<_, i64>(0)? as u64, row.get::<_, i64>(1)? as u64))
            })?,
            None => self.conn.query_row(query_main, [], |row| {
                Ok((row.get::<_, i64>(0)? as u64, row.get::<_, i64>(1)? as u64))
            })?,
        };

        let (subagent_sessions, subagent_tokens) = match project_path {
            Some(pp) => self.conn.query_row(query_sub, params![pp], |row| {
                Ok((row.get::<_, i64>(0)? as u64, row.get::<_, i64>(1)? as u64))
            })?,
            None => self.conn.query_row(query_sub, [], |row| {
                Ok((row.get::<_, i64>(0)? as u64, row.get::<_, i64>(1)? as u64))
            })?,
        };

        Ok((main_sessions, subagent_sessions, main_tokens, subagent_tokens))
    }

    /// Query all sessions, optionally filtered by project
    pub fn all_sessions(
        &self,
        project_path: Option<&str>,
    ) -> Result<Vec<SessionRow>> {
        let query = match project_path {
            Some(_) => {
                "SELECT session_id, project_path, is_subagent, agent_id,
                        first_timestamp, last_timestamp,
                        total_turns, total_input_tokens, total_output_tokens,
                        total_cache_creation_tokens, total_cache_read_tokens,
                        cache_hit_rate, total_cost_usd
                 FROM sessions
                 WHERE project_path = ?1
                 ORDER BY first_timestamp DESC"
            }
            None => {
                "SELECT session_id, project_path, is_subagent, agent_id,
                        first_timestamp, last_timestamp,
                        total_turns, total_input_tokens, total_output_tokens,
                        total_cache_creation_tokens, total_cache_read_tokens,
                        cache_hit_rate, total_cost_usd
                 FROM sessions
                 ORDER BY first_timestamp DESC"
            }
        };

        let mut stmt = self.conn.prepare(query)?;

        let rows = match project_path {
            Some(pp) => stmt.query_map(params![pp], row_to_session_row)?,
            None => stmt.query_map([], row_to_session_row)?,
        };

        rows.collect::<Result<Vec<_>, _>>()
            .context("Failed to query all sessions")
    }

    /// Query sessions that use a minimum number of distinct models
    /// Returns Vec<(session_id, model_count)>
    pub fn sessions_with_model_count(
        &self,
        min_models: u32,
    ) -> Result<Vec<(String, u32)>> {
        let mut stmt = self.conn.prepare(
            "SELECT session_id, COUNT(DISTINCT model) as model_count
             FROM session_models
             GROUP BY session_id
             HAVING model_count >= ?1
             ORDER BY model_count DESC",
        )?;

        let rows = stmt.query_map(params![min_models], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i32>(1)? as u32,
            ))
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .context("Failed to query sessions with model count")
    }

    /// Query top sessions by cost for a specific project
    pub fn top_sessions_by_cost_for_project(
        &self,
        project_path: &str,
        limit: u32,
    ) -> Result<Vec<SessionRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT session_id, project_path, is_subagent, agent_id,
                    first_timestamp, last_timestamp,
                    total_turns, total_input_tokens, total_output_tokens,
                    total_cache_creation_tokens, total_cache_read_tokens,
                    cache_hit_rate, total_cost_usd
             FROM sessions
             WHERE project_path = ?1
             ORDER BY total_cost_usd DESC
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![project_path, limit], row_to_session_row)?;
        rows.collect::<Result<Vec<_>, _>>()
            .context("Failed to query top sessions by cost for project")
    }

    /// Query daily trend for a specific month (YYYY-MM format), optionally filtered by project
    pub fn daily_trend_for_month(
        &self,
        month: &str,
        project_path: Option<&str>,
    ) -> Result<Vec<DailyStats>> {
        let query = match project_path {
            Some(_) => {
                "SELECT
                    DATE(first_timestamp) as date,
                    COALESCE(SUM(total_input_tokens), 0) as total_input,
                    COALESCE(SUM(total_output_tokens), 0) as total_output,
                    COALESCE(SUM(total_cache_creation_tokens), 0) as total_cache_creation,
                    COALESCE(SUM(total_cache_read_tokens), 0) as total_cache_read,
                    COALESCE(SUM(total_cost_usd), 0.0) as total_cost,
                    COUNT(*) as session_count
                 FROM sessions
                 WHERE project_path = ?1
                   AND first_timestamp IS NOT NULL
                   AND strftime('%Y-%m', first_timestamp) = ?2
                 GROUP BY DATE(first_timestamp)
                 ORDER BY date ASC"
            }
            None => {
                "SELECT
                    DATE(first_timestamp) as date,
                    COALESCE(SUM(total_input_tokens), 0) as total_input,
                    COALESCE(SUM(total_output_tokens), 0) as total_output,
                    COALESCE(SUM(total_cache_creation_tokens), 0) as total_cache_creation,
                    COALESCE(SUM(total_cache_read_tokens), 0) as total_cache_read,
                    COALESCE(SUM(total_cost_usd), 0.0) as total_cost,
                    COUNT(*) as session_count
                 FROM sessions
                 WHERE first_timestamp IS NOT NULL
                   AND strftime('%Y-%m', first_timestamp) = ?2
                 GROUP BY DATE(first_timestamp)
                 ORDER BY date ASC"
            }
        };

        let mut stmt = self.conn.prepare(query)?;

        let rows = match project_path {
            Some(pp) => stmt.query_map(params![pp, month], row_to_daily_stats)?,
            None => stmt.query_map(params![rusqlite::types::Null, month], row_to_daily_stats)?,
        };

        rows.collect::<Result<Vec<_>, _>>()
            .context("Failed to query daily trend for month")
    }

    /// List projects for a specific month (YYYY-MM format)
    pub fn list_projects_for_month(
        &self,
        month: &str,
        project_path: Option<&str>,
    ) -> Result<Vec<ProjectSummary>> {
        let query = match project_path {
            Some(_) => {
                "SELECT
                    project_path,
                    COUNT(*) as session_count,
                    COALESCE(SUM(total_cost_usd), 0.0) as total_cost_usd
                 FROM sessions
                 WHERE project_path = ?1
                   AND first_timestamp IS NOT NULL
                   AND strftime('%Y-%m', first_timestamp) = ?2
                 GROUP BY project_path
                 ORDER BY total_cost_usd DESC"
            }
            None => {
                "SELECT
                    project_path,
                    COUNT(*) as session_count,
                    COALESCE(SUM(total_cost_usd), 0.0) as total_cost_usd
                 FROM sessions
                 WHERE first_timestamp IS NOT NULL
                   AND strftime('%Y-%m', first_timestamp) = ?2
                 GROUP BY project_path
                 ORDER BY total_cost_usd DESC"
            }
        };

        let mut stmt = self.conn.prepare(query)?;

        let row_mapper = |row: &rusqlite::Row| -> rusqlite::Result<ProjectSummary> {
            Ok(ProjectSummary {
                project_path: row.get(0)?,
                session_count: row.get::<_, i64>(1)? as u64,
                total_cost_usd: row.get(2)?,
            })
        };

        let results: Vec<ProjectSummary> = match project_path {
            Some(pp) => stmt
                .query_map(params![pp, month], row_mapper)?
                .collect::<Result<Vec<_>, _>>()?,
            None => stmt
                .query_map(params![rusqlite::types::Null, month], row_mapper)?
                .collect::<Result<Vec<_>, _>>()?,
        };

        Ok(results)
    }

    /// Query model distribution for a specific month, optionally filtered by project
    pub fn model_distribution_for_month(
        &self,
        month: &str,
        project_path: Option<&str>,
    ) -> Result<Vec<(String, u64, f64, u64)>> {
        let query = match project_path {
            Some(_) => {
                "SELECT sm.model,
                        SUM(sm.total_input + sm.total_output + sm.total_cache_creation + sm.total_cache_read) as total_tokens,
                        COALESCE(SUM(sm.cost_usd), 0.0) as total_cost,
                        COUNT(DISTINCT sm.session_id) as session_count
                 FROM session_models sm
                 JOIN sessions s ON sm.session_id = s.session_id
                 WHERE s.project_path = ?1
                   AND s.first_timestamp IS NOT NULL
                   AND strftime('%Y-%m', s.first_timestamp) = ?2
                 GROUP BY sm.model
                 ORDER BY total_tokens DESC"
            }
            None => {
                "SELECT sm.model,
                        SUM(sm.total_input + sm.total_output + sm.total_cache_creation + sm.total_cache_read) as total_tokens,
                        COALESCE(SUM(sm.cost_usd), 0.0) as total_cost,
                        COUNT(DISTINCT sm.session_id) as session_count
                 FROM session_models sm
                 JOIN sessions s ON sm.session_id = s.session_id
                 WHERE s.first_timestamp IS NOT NULL
                   AND strftime('%Y-%m', s.first_timestamp) = ?2
                 GROUP BY sm.model
                 ORDER BY total_tokens DESC"
            }
        };

        let mut stmt = self.conn.prepare(query)?;

        let row_mapper =
            |row: &rusqlite::Row| -> rusqlite::Result<(String, u64, f64, u64)> {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)? as u64,
                    row.get::<_, f64>(2)?,
                    row.get::<_, i64>(3)? as u64,
                ))
            };

        let results: Vec<(String, u64, f64, u64)> = match project_path {
            Some(pp) => stmt
                .query_map(params![pp, month], row_mapper)?
                .collect::<Result<Vec<_>, _>>()?,
            None => stmt
                .query_map(params![rusqlite::types::Null, month], row_mapper)?
                .collect::<Result<Vec<_>, _>>()?,
        };

        Ok(results)
    }
}

// === Helper functions ===

/// Map a row to a SessionRow
fn row_to_session_row(row: &rusqlite::Row) -> rusqlite::Result<SessionRow> {
    Ok(SessionRow {
        session_id: row.get(0)?,
        project_path: row.get(1)?,
        is_subagent: row.get::<_, i32>(2)? != 0,
        agent_id: row.get(3)?,
        first_timestamp: row.get(4)?,
        last_timestamp: row.get(5)?,
        total_turns: row.get::<_, i64>(6)? as u64,
        total_input_tokens: row.get::<_, i64>(7)? as u64,
        total_output_tokens: row.get::<_, i64>(8)? as u64,
        total_cache_creation_tokens: row.get::<_, i64>(9)? as u64,
        total_cache_read_tokens: row.get::<_, i64>(10)? as u64,
        cache_hit_rate: row.get(11)?,
        total_cost_usd: row.get(12)?,
    })
}

/// Map a row to DailyStats
fn row_to_daily_stats(row: &rusqlite::Row) -> rusqlite::Result<DailyStats> {
    Ok(DailyStats {
        date: row.get(0)?,
        total_input: row.get::<_, i64>(1)? as u64,
        total_output: row.get::<_, i64>(2)? as u64,
        total_cache_creation: row.get::<_, i64>(3)? as u64,
        total_cache_read: row.get::<_, i64>(4)? as u64,
        total_cost: row.get(5)?,
        session_count: row.get::<_, i64>(6)? as u64,
    })
}

/// Walk a directory tree to find all .jsonl files
fn walk_jsonl_files(base_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut result = Vec::new();

    if !base_dir.exists() {
        return Ok(result);
    }

    walk_dir_recursive(base_dir, &mut result)?;
    Ok(result)
}

/// Recursively walk directories collecting .jsonl files
fn walk_dir_recursive(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("Failed to read directory: {}", dir.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            walk_dir_recursive(&path, out)?;
        } else if file_type.is_file()
            && path.extension().is_some_and(|ext| ext == "jsonl")
        {
            out.push(path);
        }
    }

    Ok(())
}

/// Get file modification time as seconds since epoch
fn file_mtime(path: &Path) -> Result<i64> {
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("Failed to read metadata for {}", path.display()))?;
    let mtime = metadata
        .modified()
        .context("Failed to get modification time")?;
    let duration = mtime
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    Ok(duration.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CompressionEvent, ModelUsage, ParseResult, ToolUsageStat};
    use chrono::{TimeZone, Utc};

    /// Create a minimal ParseResult for testing
    fn make_parse_result(
        session_id: &str,
        project_path: &str,
        input: u64,
        output: u64,
        cache_creation: u64,
        cache_read: u64,
        timestamp_str: &str,
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
                model: "claude-sonnet-4-20250514".to_string(),
                turn_count: 1,
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
            total_turns: 1,
            total_input_tokens: input,
            total_output_tokens: output,
            total_cache_creation_tokens: cache_creation,
            total_cache_read_tokens: cache_read,
            failed_lines: 0,
            total_lines: 10,
            turn_durations_ms: vec![500, 1200],
        }
    }

    fn test_pricing() -> PricingTable {
        PricingTable::embedded()
    }

    #[test]
    fn test_open_in_memory() {
        let db = Database::open_in_memory();
        assert!(db.is_ok(), "open_in_memory should succeed");
    }

    #[test]
    fn test_upsert_and_query() {
        let db = Database::open_in_memory().unwrap();
        let pricing = test_pricing();

        let result = make_parse_result(
            "sess-001",
            "/project/alpha",
            1000,
            500,
            200,
            800,
            "2026-03-20T10:00:00Z",
        );

        db.upsert_session(&result, &pricing).unwrap();

        let sessions = db.sessions_by_project("/project/alpha").unwrap();
        assert_eq!(sessions.len(), 1);

        let s = &sessions[0];
        assert_eq!(s.session_id, "sess-001");
        assert_eq!(s.project_path, "/project/alpha");
        assert!(!s.is_subagent);
        assert!(s.agent_id.is_none());
        assert_eq!(s.total_turns, 1);
        assert_eq!(s.total_input_tokens, 1000);
        assert_eq!(s.total_output_tokens, 500);
        assert_eq!(s.total_cache_creation_tokens, 200);
        assert_eq!(s.total_cache_read_tokens, 800);

        // cache_hit_rate = 800 / (1000 + 800 + 200) = 0.4
        assert!(s.cache_hit_rate.is_some());
        let chr = s.cache_hit_rate.unwrap();
        assert!(
            (chr - 0.4).abs() < 0.001,
            "cache_hit_rate should be 0.4, got {}",
            chr
        );

        // Cost should be positive
        assert!(s.total_cost_usd.is_some());
        assert!(s.total_cost_usd.unwrap() > 0.0);
    }

    #[test]
    fn test_upsert_replaces() {
        let db = Database::open_in_memory().unwrap();
        let pricing = test_pricing();

        // First insert
        let result1 = make_parse_result(
            "sess-replace",
            "/project/beta",
            100,
            50,
            0,
            0,
            "2026-03-20T10:00:00Z",
        );
        db.upsert_session(&result1, &pricing).unwrap();

        // Second insert with same session_id but different data
        let result2 = make_parse_result(
            "sess-replace",
            "/project/beta",
            9999,
            8888,
            7777,
            6666,
            "2026-03-20T11:00:00Z",
        );
        db.upsert_session(&result2, &pricing).unwrap();

        let sessions = db.sessions_by_project("/project/beta").unwrap();
        assert_eq!(
            sessions.len(),
            1,
            "Should have only one session after replace"
        );

        let s = &sessions[0];
        assert_eq!(s.total_input_tokens, 9999, "Should have updated data");
        assert_eq!(s.total_output_tokens, 8888);
    }

    #[test]
    fn test_global_stats() {
        let db = Database::open_in_memory().unwrap();
        let pricing = test_pricing();

        let r1 = make_parse_result(
            "sess-g1",
            "/project/a",
            1000,
            500,
            200,
            800,
            "2026-03-20T10:00:00Z",
        );
        let r2 = make_parse_result(
            "sess-g2",
            "/project/b",
            2000,
            1000,
            400,
            1600,
            "2026-03-20T11:00:00Z",
        );
        let r3 = make_parse_result(
            "sess-g3",
            "/project/a",
            500,
            250,
            100,
            400,
            "2026-03-20T12:00:00Z",
        );

        db.upsert_session(&r1, &pricing).unwrap();
        db.upsert_session(&r2, &pricing).unwrap();
        db.upsert_session(&r3, &pricing).unwrap();

        let stats = db.global_stats().unwrap();
        assert_eq!(stats.total_sessions, 3);
        assert_eq!(stats.total_projects, 2);
        assert_eq!(stats.total_input_tokens, 3500); // 1000+2000+500
        assert_eq!(stats.total_output_tokens, 1750); // 500+1000+250
        assert_eq!(stats.total_cache_creation_tokens, 700); // 200+400+100
        assert_eq!(stats.total_cache_read_tokens, 2800); // 800+1600+400
        assert_eq!(
            stats.total_tokens,
            3500 + 1750 + 700 + 2800,
            "total_tokens should be sum of all token types"
        );
        assert!(stats.total_cost_usd > 0.0);
    }

    #[test]
    fn test_project_stats() {
        let db = Database::open_in_memory().unwrap();
        let pricing = test_pricing();

        let r1 = make_parse_result(
            "sess-p1",
            "/project/x",
            1000,
            500,
            200,
            800,
            "2026-03-20T10:00:00Z",
        );
        let r2 = make_parse_result(
            "sess-p2",
            "/project/x",
            2000,
            1000,
            400,
            1600,
            "2026-03-20T11:00:00Z",
        );
        let r3 = make_parse_result(
            "sess-p3",
            "/project/y",
            500,
            250,
            100,
            400,
            "2026-03-20T12:00:00Z",
        );

        db.upsert_session(&r1, &pricing).unwrap();
        db.upsert_session(&r2, &pricing).unwrap();
        db.upsert_session(&r3, &pricing).unwrap();

        let stats = db.project_stats("/project/x").unwrap();
        assert_eq!(stats.project_path, "/project/x");
        assert_eq!(stats.session_count, 2);
        assert_eq!(
            stats.total_tokens,
            (1000 + 500 + 200 + 800) + (2000 + 1000 + 400 + 1600)
        );
        assert!(stats.total_cost_usd > 0.0);
    }

    #[test]
    fn test_daily_trend() {
        let db = Database::open_in_memory().unwrap();
        let pricing = test_pricing();

        // Sessions on different dates
        let r1 = make_parse_result(
            "sess-d1",
            "/project/t",
            1000,
            500,
            0,
            0,
            "2026-03-18T10:00:00Z",
        );
        let r2 = make_parse_result(
            "sess-d2",
            "/project/t",
            2000,
            1000,
            0,
            0,
            "2026-03-19T10:00:00Z",
        );
        let r3 = make_parse_result(
            "sess-d3",
            "/project/t",
            3000,
            1500,
            0,
            0,
            "2026-03-19T14:00:00Z",
        );

        db.upsert_session(&r1, &pricing).unwrap();
        db.upsert_session(&r2, &pricing).unwrap();
        db.upsert_session(&r3, &pricing).unwrap();

        // Query with large window to get all
        let trend = db.daily_trend(Some("/project/t"), 365).unwrap();
        assert_eq!(trend.len(), 2, "Should have 2 distinct dates");

        // First date (2026-03-18)
        let day1 = &trend[0];
        assert_eq!(day1.date, "2026-03-18");
        assert_eq!(day1.total_input, 1000);
        assert_eq!(day1.session_count, 1);

        // Second date (2026-03-19) — two sessions merged
        let day2 = &trend[1];
        assert_eq!(day2.date, "2026-03-19");
        assert_eq!(day2.total_input, 5000); // 2000 + 3000
        assert_eq!(day2.session_count, 2);
    }

    #[test]
    fn test_sync_state() {
        let db = Database::open_in_memory().unwrap();
        let path = Path::new("/test/file.jsonl");

        // Initially not synced
        let result = db.conn.query_row(
            "SELECT mtime_secs FROM sync_state WHERE file_path = ?1",
            params![path.to_string_lossy().as_ref()],
            |row| row.get::<_, i64>(0),
        );
        assert!(result.is_err(), "Should not have sync state initially");

        // Update sync state
        db.update_sync_state(path, 1234567890).unwrap();

        // Now should exist
        let mtime: i64 = db
            .conn
            .query_row(
                "SELECT mtime_secs FROM sync_state WHERE file_path = ?1",
                params![path.to_string_lossy().as_ref()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(mtime, 1234567890);

        // Update again with new mtime
        db.update_sync_state(path, 9999999999).unwrap();
        let mtime2: i64 = db
            .conn
            .query_row(
                "SELECT mtime_secs FROM sync_state WHERE file_path = ?1",
                params![path.to_string_lossy().as_ref()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(mtime2, 9999999999);
    }

    #[test]
    fn test_top_sessions_by_cost() {
        let db = Database::open_in_memory().unwrap();
        let pricing = test_pricing();

        // Insert sessions with varying token amounts (hence varying costs)
        let r1 = make_parse_result(
            "sess-cheap",
            "/project/c",
            100,
            50,
            0,
            0,
            "2026-03-20T10:00:00Z",
        );
        let r2 = make_parse_result(
            "sess-expensive",
            "/project/c",
            100_000,
            50_000,
            10_000,
            80_000,
            "2026-03-20T11:00:00Z",
        );
        let r3 = make_parse_result(
            "sess-medium",
            "/project/c",
            10_000,
            5_000,
            1_000,
            8_000,
            "2026-03-20T12:00:00Z",
        );

        db.upsert_session(&r1, &pricing).unwrap();
        db.upsert_session(&r2, &pricing).unwrap();
        db.upsert_session(&r3, &pricing).unwrap();

        let top2 = db.top_sessions_by_cost(2).unwrap();
        assert_eq!(top2.len(), 2, "Should return exactly 2 sessions");

        // Most expensive first
        assert_eq!(top2[0].session_id, "sess-expensive");
        assert_eq!(top2[1].session_id, "sess-medium");

        // Costs should be descending
        assert!(
            top2[0].total_cost_usd.unwrap() >= top2[1].total_cost_usd.unwrap(),
            "Sessions should be ordered by cost descending"
        );
    }

    #[test]
    fn test_list_projects() {
        let db = Database::open_in_memory().unwrap();
        let pricing = test_pricing();

        let r1 = make_parse_result(
            "sess-lp1",
            "/project/alpha",
            1000,
            500,
            0,
            0,
            "2026-03-20T10:00:00Z",
        );
        let r2 = make_parse_result(
            "sess-lp2",
            "/project/alpha",
            2000,
            1000,
            0,
            0,
            "2026-03-20T11:00:00Z",
        );
        let r3 = make_parse_result(
            "sess-lp3",
            "/project/beta",
            500,
            250,
            0,
            0,
            "2026-03-20T12:00:00Z",
        );

        db.upsert_session(&r1, &pricing).unwrap();
        db.upsert_session(&r2, &pricing).unwrap();
        db.upsert_session(&r3, &pricing).unwrap();

        let projects = db.list_projects().unwrap();
        assert_eq!(projects.len(), 2);

        // Ordered by cost descending — alpha has more tokens/cost
        assert_eq!(projects[0].project_path, "/project/alpha");
        assert_eq!(projects[0].session_count, 2);
        assert_eq!(projects[1].project_path, "/project/beta");
        assert_eq!(projects[1].session_count, 1);
    }

    #[test]
    fn test_global_tool_ranking() {
        let db = Database::open_in_memory().unwrap();
        let pricing = test_pricing();

        // make_parse_result adds Read=3, Bash=2 for each session
        let r1 = make_parse_result(
            "sess-tr1",
            "/project/tools",
            100,
            50,
            0,
            0,
            "2026-03-20T10:00:00Z",
        );
        let r2 = make_parse_result(
            "sess-tr2",
            "/project/tools",
            100,
            50,
            0,
            0,
            "2026-03-20T11:00:00Z",
        );

        db.upsert_session(&r1, &pricing).unwrap();
        db.upsert_session(&r2, &pricing).unwrap();

        let ranking = db.global_tool_ranking().unwrap();
        assert!(!ranking.is_empty());

        // Read: 3+3=6, Bash: 2+2=4
        let read_tool = ranking.iter().find(|t| t.name == "Read").unwrap();
        assert_eq!(read_tool.invocation_count, 6);

        let bash_tool = ranking.iter().find(|t| t.name == "Bash").unwrap();
        assert_eq!(bash_tool.invocation_count, 4);

        // Read should be first (highest count)
        assert_eq!(ranking[0].name, "Read");
    }

    #[test]
    fn test_upsert_with_compression_events() {
        let db = Database::open_in_memory().unwrap();
        let pricing = test_pricing();

        let ts = Utc.with_ymd_and_hms(2026, 3, 20, 10, 0, 0).unwrap();
        let mut result = make_parse_result(
            "sess-ce",
            "/project/ce",
            1000,
            500,
            0,
            0,
            "2026-03-20T10:00:00Z",
        );
        result.compression_events = vec![CompressionEvent {
            turn_index: 5,
            timestamp: ts,
            cache_read_before: 10000,
            cache_read_after: 500,
            drop_percentage: 0.95,
        }];

        db.upsert_session(&result, &pricing).unwrap();

        // Verify compression event was stored
        let count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM compression_events WHERE session_id = 'sess-ce'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        // Verify details
        let (turn_idx, drop_pct): (i64, f64) = db.conn.query_row(
            "SELECT turn_index, drop_percentage FROM compression_events WHERE session_id = 'sess-ce'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap();
        assert_eq!(turn_idx, 5);
        assert!((drop_pct - 0.95).abs() < 0.001);
    }

    #[test]
    fn test_upsert_with_turn_durations() {
        let db = Database::open_in_memory().unwrap();
        let pricing = test_pricing();

        let result = make_parse_result(
            "sess-td",
            "/project/td",
            1000,
            500,
            0,
            0,
            "2026-03-20T10:00:00Z",
        );
        // make_parse_result adds turn_durations_ms: [500, 1200]

        db.upsert_session(&result, &pricing).unwrap();

        let count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM turn_durations WHERE session_id = 'sess-td'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_cache_hit_rate_zero_denominator() {
        let db = Database::open_in_memory().unwrap();
        let pricing = test_pricing();

        // All token counts zero — cache_hit_rate should be None
        let result = make_parse_result(
            "sess-zero",
            "/project/zero",
            0,
            0,
            0,
            0,
            "2026-03-20T10:00:00Z",
        );

        db.upsert_session(&result, &pricing).unwrap();

        let sessions = db.sessions_by_project("/project/zero").unwrap();
        assert_eq!(sessions.len(), 1);
        assert!(
            sessions[0].cache_hit_rate.is_none(),
            "cache_hit_rate should be None when denominator is 0"
        );
    }
}
