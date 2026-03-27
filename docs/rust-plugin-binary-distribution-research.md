# Rust MCP Server Binary Distribution in Claude Code Plugins

Research date: 2026-03-27

## Executive Summary

**No Claude Code plugin in the wild ships a Rust MCP server binary inside the plugin package itself.** The plugin ecosystem is dominated by TypeScript/Python (the official marketplace is 37% Python, 33% TypeScript, 15% Shell). Compiled-language tools exist but distribute binaries externally, not through the plugin system.

The four approaches you asked about map to the ecosystem as follows:

| Approach | Used in practice? | By whom? |
|---|---|---|
| Prebuilt binaries via GitHub Releases | Yes (dominant for compiled tools) | CCometixLine, claudia-statusline, ccboard, cc-tools, Roblox Studio MCP |
| SessionStart hook to auto-build | Not found for Rust compilation | Only seen for `npm install` in official docs |
| Manual `cargo build` | Yes (as fallback/developer option) | rust-analyzer-mcp, ccboard |
| Wrapper script checking binary existence | Yes (install scripts) | claudia-statusline (`quick-install.sh`), ccboard (`install.sh`) |

---

## Repos Analyzed

### 1. CCometixLine (Rust, statusline tool)
- **Repo**: https://github.com/Haleclipse/CCometixLine
- **Binary distribution**: npm package (`@cometix/ccline`) + GitHub Releases (multi-platform)
- **Plugin structure**: None (`.claude-plugin` not used). Integrates via `settings.json` statusLine config
- **Build**: `cargo build --release` offered as developer-only option
- **Pattern**: External binary, user installs via npm or downloads release

### 2. claudia-statusline (Rust, statusline tool)
- **Repo**: https://github.com/hagan/claudia-statusline
- **Binary distribution**: GitHub Releases (multi-platform `.tar.gz`/`.zip`) + install script
- **Plugin structure**: Standalone binary, not a Claude Code plugin
- **Build**: GitHub Actions CI produces release binaries
- **Pattern**: `quick-install.sh` detects OS/arch, downloads binary to `~/.local/bin/`, auto-configures Claude Code settings

### 3. ccboard (Rust, TUI + web monitoring)
- **Repo**: https://github.com/FlorianBruniaux/ccboard
- **Binary distribution**: 4 methods -- Homebrew tap, GitHub Releases, `cargo install`, install script
- **Plugin structure**: Has `.claude-plugin` and `.claude` dirs in repo, but binary is installed externally
- **Build**: `cargo install ccboard` (crates.io) -- but warns Web UI won't work this way (skips WASM step)
- **Pattern**: Multi-channel distribution; Homebrew recommended; `ccboard setup` injects hooks into settings.json

### 4. cc-tools (Go, hooks + statusline)
- **Repo**: https://github.com/Veraticus/cc-tools
- **Binary distribution**: GitHub Releases (platform-specific archives)
- **Plugin structure**: Not a plugin. User copies binary to `~/.claude/bin/`
- **Build**: Makefile-driven (`make build`) for developers
- **Pattern**: Download release binary, copy to PATH, configure in settings.json

### 5. rust-analyzer-mcp (Rust, MCP server)
- **Repo**: https://github.com/zeenix/rust-analyzer-mcp
- **Binary distribution**: `cargo install rust-analyzer-mcp` (crates.io)
- **Plugin structure**: NOT a Claude Code plugin. User adds to `.mcp.json` manually
- **Build**: Source build via `cargo build --release` as alternative
- **Pattern**: Standard Rust crate distribution via crates.io

### 6. Roblox Studio MCP Server (Rust, MCP server)
- **Repo**: https://github.com/Roblox/studio-rust-mcp-server
- **Binary distribution**: GitHub Releases (Windows `.exe`, macOS `.zip`)
- **Plugin structure**: NOT a Claude Code plugin. Configures as MCP server in Claude Desktop
- **Build**: `cargo run` builds from source
- **Pattern**: Prebuilt binaries with automated setup script

### 7. zircote/rust-lsp (Claude Code plugin for Rust dev)
- **Repo**: https://github.com/zircote/rust-lsp
- **Binary distribution**: N/A -- this is a configuration plugin, not a compiled binary
- **Plugin structure**: Has `.claude-plugin/plugin.json`, `.lsp.json`, hooks, commands
- **Build**: Uses `/setup` command to install toolchain components (rust-analyzer, clippy, etc.)
- **Pattern**: Plugin wraps external tools; doesn't compile anything itself

### 8. Official Marketplace (anthropics/claude-plugins-official)
- **Repo**: https://github.com/anthropics/claude-plugins-official
- **200+ plugins**: ALL are TypeScript, Python, Shell, or pure markdown/config
- **Rust-related**: Only `rust-analyzer-lsp` -- which is an LSP config plugin that requires `rust-analyzer` to be pre-installed externally
- **Pattern**: Official plugins NEVER bundle compiled binaries. They either use `npx`/`node` for MCP servers, or expect system-installed tools for LSP

---

## Official Documentation Patterns

The Claude Code plugins reference (https://code.claude.com/docs/en/plugins-reference) documents:

### SessionStart Hook for Dependencies
The official pattern uses `SessionStart` to install **npm** dependencies, not compile binaries:
```json
{
  "hooks": {
    "SessionStart": [{
      "hooks": [{
        "type": "command",
        "command": "diff -q \"${CLAUDE_PLUGIN_ROOT}/package.json\" \"${CLAUDE_PLUGIN_DATA}/package.json\" >/dev/null 2>&1 || (cd \"${CLAUDE_PLUGIN_DATA}\" && cp \"${CLAUDE_PLUGIN_ROOT}/package.json\" . && npm install) || rm -f \"${CLAUDE_PLUGIN_DATA}/package.json\""
      }]
    }]
  }
}
```

### Key Environment Variables
- `${CLAUDE_PLUGIN_ROOT}` -- plugin install dir, changes on update, files don't survive updates
- `${CLAUDE_PLUGIN_DATA}` -- persistent dir at `~/.claude/plugins/data/{id}/`, survives updates

### Plugin Caching
Marketplace plugins are copied to `~/.claude/plugins/cache/`. This means any binary bundled in the repo would be copied. But the plugin system was designed for scripts/configs, not multi-MB compiled binaries.

---

## Analysis: Why No Rust Plugins Bundle Binaries

1. **Cross-platform problem**: A plugin repo would need to include binaries for macOS (Intel + ARM), Linux (x64 + ARM), and Windows. Git repos are terrible for large binaries.

2. **Plugin cache design**: Plugins are git-cloned and cached. Large binaries bloat the cache and slow installs.

3. **No official build hook pattern**: The `SessionStart` hook pattern was designed for `npm install`, not `cargo build --release` (which takes minutes, not seconds).

4. **Rust toolchain dependency**: `cargo build` requires the full Rust toolchain. Most Claude Code users don't have it installed.

---

## Viable Strategies for This Project (claude-token-analyzer)

Given that `.mcp.json` currently points to `${CLAUDE_PLUGIN_ROOT}/mcp-server/target/release/cta-mcp-server`, here are the options ordered by viability:

### Option A: SessionStart Hook with `cargo build` (simplest for developer users)
```json
{
  "hooks": {
    "SessionStart": [{
      "hooks": [{
        "type": "command",
        "command": "test -f \"${CLAUDE_PLUGIN_DATA}/cta-mcp-server\" || (cd \"${CLAUDE_PLUGIN_ROOT}/mcp-server\" && cargo build --release && cp target/release/cta-mcp-server \"${CLAUDE_PLUGIN_DATA}/\")"
      }]
    }]
  }
}
```
- Pro: Zero manual setup after install
- Con: Requires Rust toolchain; first session takes minutes; `cargo build` in a hook is unusual
- Con: The chained command pattern is complex for a hook

### Option B: Wrapper script that checks + builds (recommended hybrid)
Ship a `scripts/ensure-binary.sh` that:
1. Checks `${CLAUDE_PLUGIN_DATA}/cta-mcp-server` exists
2. If missing, tries `cargo build --release` and copies to data dir
3. Falls back to error message with install instructions

Point `.mcp.json` at the wrapper or the binary in `CLAUDE_PLUGIN_DATA`.

### Option C: GitHub Releases + install script (widest reach)
Like claudia-statusline and ccboard:
1. CI builds multi-platform binaries
2. Ship `scripts/install.sh` that detects platform, downloads binary
3. SessionStart hook runs `scripts/install.sh` if binary missing
4. `.mcp.json` points to installed binary location

### Option D: Publish to crates.io + require `cargo install` (standard Rust)
Like rust-analyzer-mcp:
1. Publish `cta-mcp-server` to crates.io
2. Users run `cargo install cta-mcp-server`
3. `.mcp.json` points to `cta-mcp-server` (must be in PATH)
4. Plugin provides skills/hooks but not the binary

### Option E: Rewrite MCP server in TypeScript/Python (ecosystem alignment)
Most Claude Code MCP servers use `npx` or `node`. The official pattern supports this natively with `SessionStart` + `npm install`. This eliminates the distribution problem entirely.

---

## Recommendation

**For a developer-focused plugin**: Option B (wrapper script) or Option C (GitHub Releases) are the most proven patterns in the ecosystem.

**For marketplace distribution**: Option C is the only approach that works for users without a Rust toolchain. Every successful Rust-based Claude Code tool (CCometixLine, claudia-statusline, ccboard) uses GitHub Releases as their primary distribution channel.

**The gap**: No one has yet solved "Rust MCP server binary bundled inside a Claude Code plugin" elegantly. This is an unsolved problem in the ecosystem. The closest pattern is the `SessionStart` + `npm install` from official docs, adapted for `cargo build`.
