//! Unified session file finder.
//!
//! Searches for JSONL session files by session ID under a projects directory,
//! including nested subagent directories. Consolidated from duplicate
//! implementations in cli.rs and mcp.rs.

use std::path::{Path, PathBuf};

/// Find a JSONL file by session ID, searching recursively under `projects_dir`.
///
/// Returns `None` if no matching file is found (callers decide how to handle:
/// cli returns an error message, mcp returns an MCP error).
pub fn find_session_file(projects_dir: &Path, session_id: &str) -> Option<PathBuf> {
    let target_filename = format!("{}.jsonl", session_id);
    walk_for_file(projects_dir, &target_filename)
}

/// Recursively walk directories looking for a file with the given name.
fn walk_for_file(dir: &Path, filename: &str) -> Option<PathBuf> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return None,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue, // skip entries with permission errors instead of aborting
        };

        if file_type.is_file() && path.file_name().and_then(|n| n.to_str()) == Some(filename) {
            return Some(path);
        }

        if file_type.is_dir()
            && let Some(found) = walk_for_file(&path, filename)
        {
            return Some(found);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Helper: create a temp dir with given structure
    fn setup_test_dir(files: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for file in files {
            let path = dir.path().join(file);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&path, "{}").unwrap();
        }
        dir
    }

    #[test]
    fn test_find_session_in_flat_dir() {
        let dir = setup_test_dir(&["abc123.jsonl"]);
        let result = find_session_file(dir.path(), "abc123");
        assert!(result.is_some());
        assert!(result.unwrap().ends_with("abc123.jsonl"));
    }

    #[test]
    fn test_find_session_in_nested_dir() {
        let dir = setup_test_dir(&["project-a/sessions/def456.jsonl"]);
        let result = find_session_file(dir.path(), "def456");
        assert!(result.is_some());
        assert!(result.unwrap().ends_with("def456.jsonl"));
    }

    #[test]
    fn test_find_session_not_found() {
        let dir = setup_test_dir(&["other.jsonl"]);
        let result = find_session_file(dir.path(), "nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn test_find_session_in_subagent_dir() {
        let dir = setup_test_dir(&["project-a/sessions/subagents/agent-abc/ghi789.jsonl"]);
        let result = find_session_file(dir.path(), "ghi789");
        assert!(result.is_some());
        assert!(result.unwrap().ends_with("ghi789.jsonl"));
    }

    #[test]
    fn test_find_session_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let result = find_session_file(dir.path(), "anything");
        assert!(result.is_none());
    }
}
