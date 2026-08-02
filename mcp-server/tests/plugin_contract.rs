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
