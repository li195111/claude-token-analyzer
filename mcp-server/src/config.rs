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
        let _guard = ENV_LOCK.lock().unwrap();
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
}
