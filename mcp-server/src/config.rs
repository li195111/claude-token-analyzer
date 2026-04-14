//! Centralized path resolution for CTA.
//!
//! Resolution priority:
//! 1. Environment variable override ($CTA_DB_PATH, $CTA_PROJECTS_DIR, $CTA_ARCHIVE_DIR)
//! 2. Plugin mode ($CLAUDE_PLUGIN_ROOT/data/...)
//! 3. Standalone mode ($HOME/.claude/...)

use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result};

/// Resolve the database file path.
///
/// Priority: `$CTA_DB_PATH` > `$CLAUDE_PLUGIN_ROOT/data/token-analyzer.db` > `$HOME/.claude/token-analyzer.db`
pub fn resolve_db_path() -> Result<PathBuf> {
    if let Ok(p) = env::var("CTA_DB_PATH") {
        return Ok(PathBuf::from(p));
    }
    if let Ok(root) = env::var("CLAUDE_PLUGIN_ROOT") {
        return Ok(PathBuf::from(root).join("data").join("token-analyzer.db"));
    }
    let home = home_dir()?;
    Ok(home.join(".claude").join("token-analyzer.db"))
}

/// Resolve the projects directory path.
///
/// Priority: `$CTA_PROJECTS_DIR` > `$HOME/.claude/projects`
///
/// Note: No plugin-mode override — projects dir is always under `~/.claude/projects`
/// because that's where Claude Code stores session data regardless of plugin installation.
pub fn resolve_projects_dir() -> Result<PathBuf> {
    if let Ok(p) = env::var("CTA_PROJECTS_DIR") {
        return Ok(PathBuf::from(p));
    }
    let home = home_dir()?;
    Ok(home.join(".claude").join("projects"))
}

/// Resolve the archive directory path.
///
/// Priority: `$CTA_ARCHIVE_DIR` > `$CLAUDE_PLUGIN_ROOT/data/token-analyzer-archive` > `$HOME/.claude/token-analyzer-archive`
pub fn resolve_archive_dir() -> Result<PathBuf> {
    if let Ok(p) = env::var("CTA_ARCHIVE_DIR") {
        return Ok(PathBuf::from(p));
    }
    if let Ok(root) = env::var("CLAUDE_PLUGIN_ROOT") {
        return Ok(PathBuf::from(root)
            .join("data")
            .join("token-analyzer-archive"));
    }
    let home = home_dir()?;
    Ok(home.join(".claude").join("token-analyzer-archive"))
}

fn home_dir() -> Result<PathBuf> {
    env::var("HOME").map(PathBuf::from).context(
        "HOME environment variable not set (and no CTA_DB_PATH / CLAUDE_PLUGIN_ROOT override)",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialize env-var tests to avoid races
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Helper: run closure with env vars set, then restore originals
    fn with_env_vars<F: FnOnce() -> R, R>(vars: &[(&str, Option<&str>)], f: F) -> R {
        // Recover from poisoned mutex so that a panicking test (e.g., a TDD red-light test
        // that intentionally fails an assertion) does not cascade and corrupt ENV state
        // for subsequent tests. Using unwrap_or_else(|e| e.into_inner()) is the standard
        // Rust pattern for poison recovery in test helpers.
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let originals: Vec<(&str, Option<String>)> =
            vars.iter().map(|(k, _)| (*k, env::var(k).ok())).collect();

        for (k, v) in vars {
            // SAFETY: tests are serialized via ENV_LOCK mutex
            unsafe {
                match v {
                    Some(val) => env::set_var(k, val),
                    None => env::remove_var(k),
                }
            }
        }

        let result = f();

        for (k, orig) in &originals {
            // SAFETY: tests are serialized via ENV_LOCK mutex
            unsafe {
                match orig {
                    Some(val) => env::set_var(k, val),
                    None => env::remove_var(k),
                }
            }
        }
        result
    }

    #[test]
    fn test_resolve_db_path_from_env() {
        with_env_vars(&[("CTA_DB_PATH", Some("/custom/db.sqlite"))], || {
            let path = resolve_db_path().unwrap();
            assert_eq!(path, PathBuf::from("/custom/db.sqlite"));
        });
    }

    #[test]
    fn test_resolve_db_path_plugin_mode() {
        with_env_vars(
            &[
                ("CTA_DB_PATH", None),
                ("CLAUDE_PLUGIN_ROOT", Some("/plugins/cta")),
            ],
            || {
                let path = resolve_db_path().unwrap();
                assert_eq!(path, PathBuf::from("/plugins/cta/data/token-analyzer.db"));
            },
        );
    }

    #[test]
    fn test_resolve_db_path_standalone() {
        with_env_vars(
            &[
                ("CTA_DB_PATH", None),
                ("CLAUDE_PLUGIN_ROOT", None),
                ("HOME", Some("/home/testuser")),
            ],
            || {
                let path = resolve_db_path().unwrap();
                assert_eq!(
                    path,
                    PathBuf::from("/home/testuser/.claude/token-analyzer.db")
                );
            },
        );
    }

    #[test]
    fn test_resolve_projects_dir_from_env() {
        with_env_vars(&[("CTA_PROJECTS_DIR", Some("/custom/projects"))], || {
            let path = resolve_projects_dir().unwrap();
            assert_eq!(path, PathBuf::from("/custom/projects"));
        });
    }

    #[test]
    fn test_resolve_projects_dir_default() {
        with_env_vars(
            &[("CTA_PROJECTS_DIR", None), ("HOME", Some("/home/testuser"))],
            || {
                let path = resolve_projects_dir().unwrap();
                assert_eq!(path, PathBuf::from("/home/testuser/.claude/projects"));
            },
        );
    }

    #[test]
    fn test_resolve_fails_without_home() {
        with_env_vars(
            &[
                ("CTA_DB_PATH", None),
                ("CLAUDE_PLUGIN_ROOT", None),
                ("HOME", None),
            ],
            || {
                let result = resolve_db_path();
                assert!(result.is_err());
                let err_msg = result.unwrap_err().to_string();
                assert!(
                    err_msg.contains("HOME"),
                    "Error should mention HOME: {}",
                    err_msg
                );
            },
        );
    }

    // ============================================================
    // PR #7 Missing Tests: resolve_archive_dir with CLAUDE_CONFIG_DIR
    // These 5 tests were identified as missing in the code review of PR #7.
    // Tests 1-4 pass with current code; test 5 fails (CLAUDE_CONFIG_DIR not yet supported).
    // ============================================================

    #[test]
    fn test_resolve_archive_dir_from_env() {
        with_env_vars(
            &[
                ("CTA_ARCHIVE_DIR", Some("/custom/archive")),
                ("CLAUDE_PLUGIN_ROOT", None),
                ("CLAUDE_CONFIG_DIR", None),
            ],
            || {
                let path = resolve_archive_dir().unwrap();
                assert_eq!(path, PathBuf::from("/custom/archive"));
            },
        );
    }

    #[test]
    fn test_resolve_archive_dir_plugin_mode() {
        with_env_vars(
            &[
                ("CTA_ARCHIVE_DIR", None),
                ("CLAUDE_PLUGIN_ROOT", Some("/plugins/cta")),
                ("CLAUDE_CONFIG_DIR", None),
            ],
            || {
                let path = resolve_archive_dir().unwrap();
                assert_eq!(
                    path,
                    PathBuf::from("/plugins/cta/data/token-analyzer-archive")
                );
            },
        );
    }

    #[test]
    fn test_resolve_archive_dir_standalone() {
        with_env_vars(
            &[
                ("CTA_ARCHIVE_DIR", None),
                ("CLAUDE_PLUGIN_ROOT", None),
                ("CLAUDE_CONFIG_DIR", None),
                ("HOME", Some("/home/testuser")),
            ],
            || {
                let path = resolve_archive_dir().unwrap();
                assert_eq!(
                    path,
                    PathBuf::from("/home/testuser/.claude/token-analyzer-archive")
                );
            },
        );
    }

    #[test]
    fn test_plugin_root_takes_priority_over_config_dir_for_archive() {
        // When BOTH CLAUDE_PLUGIN_ROOT and CLAUDE_CONFIG_DIR are set,
        // archive dir should use PLUGIN_ROOT (plugin mode takes highest priority).
        with_env_vars(
            &[
                ("CTA_ARCHIVE_DIR", None),
                ("CLAUDE_PLUGIN_ROOT", Some("/plugins/cta")),
                ("CLAUDE_CONFIG_DIR", Some("/home/user/.config/claude")),
                ("HOME", Some("/home/user")),
            ],
            || {
                let path = resolve_archive_dir().unwrap();
                assert_eq!(
                    path,
                    PathBuf::from("/plugins/cta/data/token-analyzer-archive"),
                    "CLAUDE_PLUGIN_ROOT should take priority over CLAUDE_CONFIG_DIR for archive dir"
                );
            },
        );
    }

    #[test]
    fn test_coexistence_plugin_root_and_config_dir_split_behavior() {
        // When BOTH CLAUDE_PLUGIN_ROOT and CLAUDE_CONFIG_DIR are set:
        //   db      → PLUGIN_ROOT/data/token-analyzer.db
        //   archive → PLUGIN_ROOT/data/token-analyzer-archive
        //   projects → CONFIG_DIR/projects  (NOT PLUGIN_ROOT/projects — Claude Code always writes here)
        //
        // This test FAILS with current code because resolve_projects_dir()
        // does not yet support CLAUDE_CONFIG_DIR (PR #7 feature).
        with_env_vars(
            &[
                ("CTA_DB_PATH", None),
                ("CTA_ARCHIVE_DIR", None),
                ("CTA_PROJECTS_DIR", None),
                ("CLAUDE_PLUGIN_ROOT", Some("/plugins/cta")),
                ("CLAUDE_CONFIG_DIR", Some("/home/user/.config/claude")),
                ("HOME", Some("/home/user")),
            ],
            || {
                let db_path = resolve_db_path().unwrap();
                let archive_path = resolve_archive_dir().unwrap();
                let projects_path = resolve_projects_dir().unwrap();

                assert_eq!(
                    db_path,
                    PathBuf::from("/plugins/cta/data/token-analyzer.db"),
                    "db should use PLUGIN_ROOT"
                );
                assert_eq!(
                    archive_path,
                    PathBuf::from("/plugins/cta/data/token-analyzer-archive"),
                    "archive should use PLUGIN_ROOT"
                );
                // This assertion FAILS with current code (returns HOME/.claude/projects instead).
                // It will pass after PR #7 is merged.
                assert_eq!(
                    projects_path,
                    PathBuf::from("/home/user/.config/claude/projects"),
                    "projects should use CLAUDE_CONFIG_DIR (PR #7 feature)"
                );
            },
        );
    }
}
