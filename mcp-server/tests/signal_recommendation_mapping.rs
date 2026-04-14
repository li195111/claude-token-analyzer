use std::fs;
use std::path::Path;

fn mapping_file() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("skills")
        .join("cta-usage-pattern")
        .join("references")
        .join("harness-signals-to-advice.md")
}

#[test]
fn test_mapping_file_covers_all_patterns() {
    let content = fs::read_to_string(mapping_file()).expect("mapping file should exist");

    for pattern in [
        "## cold_session",
        "## correction_spiral",
        "## subagent_swarm",
        "## kitchen_sink",
        "## marathon",
        "## observer",
        "## normal",
    ] {
        assert!(
            content.contains(pattern),
            "mapping file should contain section {pattern}"
        );
    }
}

#[test]
fn test_mapping_file_contains_key_guidance_terms() {
    let content = fs::read_to_string(mapping_file()).expect("mapping file should exist");

    for keyword in ["cache", "diff", "subagent", "checkpoint"] {
        assert!(
            content.contains(keyword),
            "mapping file should mention keyword {keyword}"
        );
    }
}
