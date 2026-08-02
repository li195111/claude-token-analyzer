use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

const RELEASE_VERSION: &str = "0.3.0";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("mcp-server must live under the repository root")
        .to_path_buf()
}

fn read(path: impl AsRef<Path>) -> String {
    let path = path.as_ref();
    fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn read_json(path: impl AsRef<Path>) -> Value {
    let path = path.as_ref();
    serde_json::from_str(&read(path))
        .unwrap_or_else(|error| panic!("parse {} as JSON: {error}", path.display()))
}

#[test]
fn runtime_versions_match_release_contract() {
    let root = repo_root();
    let plugin = read_json(root.join(".claude-plugin/plugin.json"));
    let cargo: toml::Value = toml::from_str(&read(root.join("mcp-server/Cargo.toml")))
        .expect("parse mcp-server/Cargo.toml");
    let lock: toml::Value = toml::from_str(&read(root.join("mcp-server/Cargo.lock")))
        .expect("parse mcp-server/Cargo.lock");

    assert_eq!(plugin["version"], RELEASE_VERSION);
    assert_eq!(cargo["package"]["version"].as_str(), Some(RELEASE_VERSION));

    let locked_package = lock["package"]
        .as_array()
        .expect("Cargo.lock package array")
        .iter()
        .find(|package| package["name"].as_str() == Some("claude-token-analyzer"))
        .expect("claude-token-analyzer package in Cargo.lock");
    assert_eq!(locked_package["version"].as_str(), Some(RELEASE_VERSION));

    if let Ok(tag) = std::env::var("CTA_RELEASE_TAG") {
        assert_eq!(tag, format!("v{RELEASE_VERSION}"));
    }
}

#[test]
fn marketplace_uses_plugin_manifest_as_version_authority() {
    let marketplace = read_json(repo_root().join(".claude-plugin/marketplace.json"));
    let plugin = &marketplace["plugins"][0];

    assert!(marketplace["description"].is_string());
    assert!(marketplace.get("metadata").is_none());
    assert!(plugin.get("version").is_none());
    assert_eq!(plugin["source"], "./");
}

#[test]
fn executable_inventory_is_current() {
    let root = repo_root();
    let mcp_source = read(root.join("mcp-server/src/bin/mcp.rs"));
    let tool_count = mcp_source.matches("#[tool(").count();
    assert_eq!(tool_count, 8, "MCP tool inventory changed");

    let skill_count = fs::read_dir(root.join("skills"))
        .expect("read skills directory")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("SKILL.md").is_file())
        .count();
    assert_eq!(skill_count, 7, "skill inventory changed");

    let marketplace = read_json(root.join(".claude-plugin/marketplace.json"));
    let description = marketplace["plugins"][0]["description"]
        .as_str()
        .expect("marketplace plugin description");
    assert!(description.contains("8 MCP tools + 7 workflow skills"));
}

#[test]
fn binary_distribution_is_versioned_and_single_entrypoint() {
    let root = repo_root();
    let installer = read(root.join("scripts/install.sh"));
    let runner = read(root.join("scripts/run.sh"));
    let release = read(root.join(".github/workflows/release.yml"));

    assert!(!root.join("hooks/hooks.json").exists());
    assert!(!installer.contains("releases/latest"));
    assert!(installer.contains("releases/download/v$VERSION"));
    assert!(installer.contains("INSTALL_PATH=\"$INSTALL_DIR/$BINARY_NAME-$VERSION\""));
    assert!(installer.contains("SHA256SUMS"));
    assert!(runner.contains("BINARY=\"$(bash \"$SCRIPT_DIR/install.sh\")\""));
    assert!(release.contains("cargo build --release --locked"));
    assert!(release.contains("scripts/mcp-stdio-smoke.mjs"));
    assert!(release.contains("sha256sum cta-mcp-server-* > SHA256SUMS"));
}

#[test]
fn skills_use_native_output_language_contract() {
    let root = repo_root();
    let plugin = read_json(root.join(".claude-plugin/plugin.json"));
    let language = &plugin["userConfig"]["output_language"];

    assert_eq!(language["type"], "string");
    assert_eq!(language["default"], "auto");
    assert!(language["title"].is_string());
    assert!(language["description"].is_string());
    assert!(language.get("required").is_none());
    assert!(language.get("sensitive").is_none());

    let skill_paths: Vec<_> = fs::read_dir(root.join("skills"))
        .expect("read skills directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("SKILL.md"))
        .filter(|path| path.is_file())
        .collect();
    assert_eq!(skill_paths.len(), 7);

    for path in skill_paths {
        let contents = read(&path);
        assert!(
            contents.contains("Configured output language: `${user_config.output_language}`."),
            "{} lacks the native userConfig placeholder",
            path.display()
        );
        assert!(contents.contains("literal unexpanded placeholder"));
        assert!(contents.contains("latest user message's primary natural language"));
        assert!(contents.contains("fall back to English"));
        assert!(contents.contains("technical identifiers"));
        assert!(contents.contains("the English example below defines structure only"));
        assert!(!contents.contains("Use 繁體中文"));
        assert!(!contents.contains("> 「"));

        let mut in_code_fence = false;
        for line in contents.lines() {
            if line.trim_start().starts_with("```") {
                in_code_fence = !in_code_fence;
                continue;
            }
            if in_code_fence {
                assert!(
                    !line.chars().any(|character| ('\u{4e00}'..='\u{9fff}').contains(&character)),
                    "{} contains a fixed CJK output template line: {line}",
                    path.display()
                );
            }
        }
    }

    let usage_pattern = read(root.join("skills/cta-usage-pattern/SKILL.md"));
    assert!(usage_pattern.contains("usage pattern"));
    assert!(usage_pattern.contains("workflow advice"));
}

#[test]
fn public_install_and_inventory_docs_are_current() {
    let root = repo_root();
    let readme = read(root.join("README.md"));
    let claude = read(root.join("CLAUDE.md"));
    let changelog = read(root.join("CHANGELOG.md"));

    let marketplace_add =
        "claude plugin marketplace add https://github.com/li195111/claude-token-analyzer.git";
    let qualified_install =
        "claude plugin install claude-token-analyzer@claude-token-analyzer";
    assert!(readme.matches(marketplace_add).count() >= 2);
    assert!(readme.matches(qualified_install).count() >= 2);
    assert!(readme.contains("--config output_language=en"));
    assert!(readme.contains("/plugin configure"));
    assert!(readme.contains("macOS x86_64"));
    assert!(readme.contains("macOS arm64"));
    assert!(readme.contains("Linux x86_64"));
    assert!(readme.contains("Linux arm64"));
    assert!(readme.contains("`cta-usage-pattern`"));
    assert!(readme.contains("`classify_session_pattern`"));
    assert!(readme.contains("173+ automated tests"));
    assert!(!readme.lines().any(|line| {
        line.trim() == "claude plugin install claude-token-analyzer"
    }));

    assert!(claude.contains("8 MCP tools"));
    assert!(claude.contains("7 workflow skills"));
    assert!(claude.contains("173+ tests"));
    assert!(claude.contains("--locked"));
    assert!(!claude.contains("SessionStart"));
    assert!(!claude.contains("106 tests"));

    assert!(changelog.contains("## [0.3.0] - Unreleased"));
    assert!(changelog.contains("output_language"));
    assert!(changelog.contains("SHA256SUMS"));
}
