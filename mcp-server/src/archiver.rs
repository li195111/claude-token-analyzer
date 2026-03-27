use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

/// Manages zstd-compressed archives of JSONL session files
pub struct Archiver {
    archive_dir: PathBuf,
}

/// Manifest tracking all archived files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveManifest {
    pub entries: Vec<ArchiveEntry>,
    pub last_updated: String,
}

/// A single entry in the archive manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveEntry {
    pub session_id: String,
    pub source_path: String,
    pub archive_path: String,
    pub original_size: u64,
    pub compressed_size: u64,
    pub archived_at: String,
}

impl Archiver {
    /// Create a new Archiver targeting the given archive directory
    pub fn new(archive_dir: &Path) -> Self {
        Self {
            archive_dir: archive_dir.to_path_buf(),
        }
    }

    /// Compress a JSONL file to zstd, saving to archive_dir/<project-dir-name>/<session>.jsonl.zst
    ///
    /// The project directory name is derived from the source file's parent directory name.
    /// The session name is the file stem (filename without extension).
    pub fn archive_file(&self, source: &Path) -> Result<ArchiveEntry> {
        // Derive project name and session id from path
        let project_name = source
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let session_id = source
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Build archive path
        let archive_subdir = self.archive_dir.join(project_name);
        fs::create_dir_all(&archive_subdir).with_context(|| {
            format!(
                "Failed to create archive directory: {}",
                archive_subdir.display()
            )
        })?;

        let archive_filename = format!("{}.jsonl.zst", session_id);
        let archive_path = archive_subdir.join(&archive_filename);

        // Read source file
        let source_data = fs::read(source)
            .with_context(|| format!("Failed to read source file: {}", source.display()))?;
        let original_size = source_data.len() as u64;

        // Compress with zstd level 3
        let compressed =
            zstd::encode_all(source_data.as_slice(), 3).context("Failed to compress with zstd")?;
        let compressed_size = compressed.len() as u64;

        // Write compressed file
        let mut out_file = fs::File::create(&archive_path).with_context(|| {
            format!("Failed to create archive file: {}", archive_path.display())
        })?;
        out_file
            .write_all(&compressed)
            .context("Failed to write compressed data")?;

        Ok(ArchiveEntry {
            session_id,
            source_path: source.to_string_lossy().to_string(),
            archive_path: archive_path.to_string_lossy().to_string(),
            original_size,
            compressed_size,
            archived_at: Utc::now().to_rfc3339(),
        })
    }

    /// Decompress an archived file back to the destination path
    pub fn restore_file(&self, entry: &ArchiveEntry, dest: &Path) -> Result<()> {
        // Ensure destination directory exists
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "Failed to create destination directory: {}",
                    parent.display()
                )
            })?;
        }

        // Read compressed file
        let compressed_data = fs::read(&entry.archive_path)
            .with_context(|| format!("Failed to read archive file: {}", entry.archive_path))?;

        // Decompress
        let decompressed = zstd::decode_all(compressed_data.as_slice())
            .context("Failed to decompress zstd data")?;

        // Write to destination
        fs::write(dest, &decompressed)
            .with_context(|| format!("Failed to write restored file: {}", dest.display()))?;

        Ok(())
    }

    /// Find JSONL files whose mtime indicates they will expire within `days_threshold` days.
    ///
    /// Claude Code JSONL files are typically retained for ~30 days. This method finds
    /// files older than (30 - days_threshold) days based on file modification time.
    pub fn find_expiring_sessions(source_dir: &Path, days_threshold: u32) -> Result<Vec<PathBuf>> {
        let mut expiring = Vec::new();

        if !source_dir.exists() {
            return Ok(expiring);
        }

        let now = std::time::SystemTime::now();
        // Files older than (30 - threshold) days are expiring within threshold days
        let max_age_secs: u64 = (30u64.saturating_sub(days_threshold as u64)) * 24 * 60 * 60;

        walk_jsonl_files_recursive(source_dir, &mut |path: &Path| {
            let dominated = fs::metadata(path)
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|mtime| now.duration_since(mtime).ok())
                .is_some_and(|age| age.as_secs() >= max_age_secs);

            if dominated {
                expiring.push(path.to_path_buf());
            }
        })?;

        Ok(expiring)
    }

    /// Load the archive manifest from manifest.json
    pub fn load_manifest(&self) -> Result<ArchiveManifest> {
        let manifest_path = self.archive_dir.join("manifest.json");

        if !manifest_path.exists() {
            return Ok(ArchiveManifest {
                entries: Vec::new(),
                last_updated: Utc::now().to_rfc3339(),
            });
        }

        let content = fs::read_to_string(&manifest_path)
            .with_context(|| format!("Failed to read manifest: {}", manifest_path.display()))?;

        serde_json::from_str(&content).context("Failed to parse manifest JSON")
    }

    /// Save the archive manifest to manifest.json
    pub fn save_manifest(&self, manifest: &ArchiveManifest) -> Result<()> {
        fs::create_dir_all(&self.archive_dir).with_context(|| {
            format!(
                "Failed to create archive directory: {}",
                self.archive_dir.display()
            )
        })?;

        let manifest_path = self.archive_dir.join("manifest.json");
        let content =
            serde_json::to_string_pretty(manifest).context("Failed to serialize manifest")?;

        fs::write(&manifest_path, content)
            .with_context(|| format!("Failed to write manifest: {}", manifest_path.display()))?;

        Ok(())
    }
}

/// Recursively walk directories, calling `callback` for each .jsonl file found
fn walk_jsonl_files_recursive(dir: &Path, callback: &mut dyn FnMut(&Path)) -> Result<()> {
    let entries = fs::read_dir(dir)
        .with_context(|| format!("Failed to read directory: {}", dir.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            walk_jsonl_files_recursive(&path, callback)?;
        } else if file_type.is_file()
            && path.extension().is_some_and(|ext| ext == "jsonl")
        {
            callback(&path);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Create a test JSONL file with some content
    fn create_test_jsonl(dir: &Path, project: &str, filename: &str, content: &str) -> PathBuf {
        let project_dir = dir.join(project);
        fs::create_dir_all(&project_dir).unwrap();
        let file_path = project_dir.join(filename);
        fs::write(&file_path, content).unwrap();
        file_path
    }

    #[test]
    fn test_compress_decompress_roundtrip() {
        let source_dir = TempDir::new().unwrap();
        let archive_dir = TempDir::new().unwrap();
        let restore_dir = TempDir::new().unwrap();

        // Create a test JSONL file
        let original_content = r#"{"type":"assistant","requestId":"req_001","timestamp":"2026-03-20T10:00:00Z","message":{"model":"claude-sonnet-4-20250514","content":[],"usage":{"input_tokens":100,"output_tokens":50}}}
{"type":"user","timestamp":"2026-03-20T10:00:01Z","message":{"role":"user","content":"hello"}}
{"type":"assistant","requestId":"req_002","timestamp":"2026-03-20T10:00:02Z","message":{"model":"claude-sonnet-4-20250514","content":[],"usage":{"input_tokens":200,"output_tokens":100}}}
"#;

        let source_path = create_test_jsonl(
            source_dir.path(),
            "project-abc123",
            "session-uuid-001.jsonl",
            original_content,
        );

        // Archive
        let archiver = Archiver::new(archive_dir.path());
        let entry = archiver.archive_file(&source_path).unwrap();

        assert_eq!(entry.session_id, "session-uuid-001");
        assert_eq!(entry.original_size, original_content.len() as u64);
        assert!(entry.compressed_size > 0);
        assert!(
            entry.compressed_size <= entry.original_size,
            "Compressed should be <= original (compressed={}, original={})",
            entry.compressed_size,
            entry.original_size
        );

        // Verify archive file exists
        assert!(
            Path::new(&entry.archive_path).exists(),
            "Archive file should exist at {}",
            entry.archive_path
        );

        // Restore
        let restore_path = restore_dir.path().join("restored.jsonl");
        archiver.restore_file(&entry, &restore_path).unwrap();

        // Compare bytes
        let restored_content = fs::read_to_string(&restore_path).unwrap();
        assert_eq!(
            restored_content, original_content,
            "Restored content should match original exactly"
        );
    }

    #[test]
    fn test_manifest_save_load() {
        let archive_dir = TempDir::new().unwrap();
        let archiver = Archiver::new(archive_dir.path());

        // Initially empty
        let manifest = archiver.load_manifest().unwrap();
        assert!(manifest.entries.is_empty());

        // Save a manifest with entries
        let manifest_to_save = ArchiveManifest {
            entries: vec![
                ArchiveEntry {
                    session_id: "sess-001".to_string(),
                    source_path: "/source/sess-001.jsonl".to_string(),
                    archive_path: "/archive/proj/sess-001.jsonl.zst".to_string(),
                    original_size: 10000,
                    compressed_size: 1500,
                    archived_at: "2026-03-20T10:00:00Z".to_string(),
                },
                ArchiveEntry {
                    session_id: "sess-002".to_string(),
                    source_path: "/source/sess-002.jsonl".to_string(),
                    archive_path: "/archive/proj/sess-002.jsonl.zst".to_string(),
                    original_size: 20000,
                    compressed_size: 3000,
                    archived_at: "2026-03-20T11:00:00Z".to_string(),
                },
            ],
            last_updated: "2026-03-20T12:00:00Z".to_string(),
        };

        archiver.save_manifest(&manifest_to_save).unwrap();

        // Load it back
        let loaded = archiver.load_manifest().unwrap();
        assert_eq!(loaded.entries.len(), 2);
        assert_eq!(loaded.entries[0].session_id, "sess-001");
        assert_eq!(loaded.entries[0].original_size, 10000);
        assert_eq!(loaded.entries[0].compressed_size, 1500);
        assert_eq!(loaded.entries[1].session_id, "sess-002");
        assert_eq!(loaded.last_updated, "2026-03-20T12:00:00Z");
    }

    #[test]
    fn test_find_expiring_sessions() {
        let source_dir = TempDir::new().unwrap();

        // Create some JSONL files
        let _file1 =
            create_test_jsonl(source_dir.path(), "project-a", "sess-new.jsonl", "content");

        // Create a file and set its mtime to 26 days ago (within 30-day window,
        // should be found with threshold=25 since 30-25=5 days, and 26 > 5)
        let file2 = create_test_jsonl(
            source_dir.path(),
            "project-b",
            "sess-old.jsonl",
            "old content",
        );

        // Set mtime to 26 days ago
        let old_time =
            std::time::SystemTime::now() - std::time::Duration::from_secs(26 * 24 * 60 * 60);
        filetime::set_file_mtime(&file2, filetime::FileTime::from_system_time(old_time))
            .unwrap();

        // Find expiring files (files older than 30-25=5 days)
        let expiring = Archiver::find_expiring_sessions(source_dir.path(), 25).unwrap();

        // The old file should be found (26 days old > 5 day threshold)
        // The new file might or might not be found depending on timing
        // Since we just created file1 it should NOT be expiring
        let old_found = expiring.iter().any(|p| p.ends_with("sess-old.jsonl"));
        assert!(
            old_found,
            "Old file should be found as expiring. Found: {:?}",
            expiring
        );
    }

    #[test]
    fn test_find_expiring_nonexistent_dir() {
        let result =
            Archiver::find_expiring_sessions(Path::new("/nonexistent/path/that/doesnt/exist"), 25);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_archive_creates_subdirectory() {
        let source_dir = TempDir::new().unwrap();
        let archive_dir = TempDir::new().unwrap();

        let source_path = create_test_jsonl(
            source_dir.path(),
            "my-project-hash",
            "session-123.jsonl",
            "test data",
        );

        let archiver = Archiver::new(archive_dir.path());
        let entry = archiver.archive_file(&source_path).unwrap();

        // Should create subdirectory matching project name
        let expected_dir = archive_dir.path().join("my-project-hash");
        assert!(
            expected_dir.exists(),
            "Project subdirectory should be created"
        );
        assert!(
            entry.archive_path.contains("my-project-hash"),
            "Archive path should contain project name"
        );
        assert!(
            entry.archive_path.ends_with(".jsonl.zst"),
            "Archive should have .jsonl.zst extension"
        );
    }
}
