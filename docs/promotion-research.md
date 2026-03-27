# Claude Code Plugin / MCP Server Promotion Research

**Date:** 2026-03-27
**Subject:** Market research for promoting `claude-token-analyzer`

---

## 1. Where Claude Code Users Discover Plugins

### 1.1 Official Anthropic Marketplace (Primary Channel)

The official marketplace at `anthropics/claude-plugins-official` on GitHub is the primary discovery channel. It comes pre-configured with Claude Code, so users can browse and install plugins immediately via `/plugin > Discover`.

- **Install command:** `/plugin install {plugin-name}@claude-plugins-official`
- **Submission:** External plugins must meet quality and security standards. Submit via the [plugin directory submission form](https://clau.de/plugin-directory-submission) or at `platform.claude.com/plugins/submit`.
- **Verified badge:** Plugins with an "Anthropic Verified" badge have undergone additional quality and safety review.
- **This is the highest-impact channel** because it is built into the Claude Code UI.

### 1.2 Claude.com Plugins Page

Anthropic maintains a public plugins page at [claude.com/plugins](https://claude.com/plugins) where users can discover and share plugins for Claude Code and Cowork.

### 1.3 Community Marketplaces (Git-based)

Anyone can create a marketplace by hosting a Git repo with a `.claude-plugin/marketplace.json` file. Notable third-party marketplaces:

| Marketplace | URL | How to Add |
|---|---|---|
| **Build with Claude** | [buildwithclaude.com](https://buildwithclaude.com/) | `/plugin marketplace add davepoon/buildwithclaude` |
| **claude-tools (paddo)** | [paddo.dev](https://paddo.dev/blog/claude-tools-plugin-marketplace/) | `/plugin marketplace add paddo/claude-tools` |
| **cc-marketplace** | [github.com/ananddtyagi/cc-marketplace](https://github.com/ananddtyagi/cc-marketplace) | `/plugin marketplace add ananddtyagi/cc-marketplace` |
| **claude-plugins.dev** | [claude-plugins.dev](https://claude-plugins.dev/) | Community registry with CLI |

### 1.4 Aggregator/Directory Websites

These websites scrape or accept submissions and help users discover plugins:

| Site | Focus | Submission |
|---|---|---|
| [claudemarketplaces.com](https://claudemarketplaces.com/) | Plugin & marketplace directory | Auto-discovered from GitHub repos with `marketplace.json`; requires 500+ installs to be listed |
| [claudepluginhub.com](https://www.claudepluginhub.com/) | Community directory | Submission-based |
| [awesomeclaude.ai](https://awesomeclaude.ai/) | Claude resource directory | Submission-based |

### 1.5 MCP Server Directories (for the MCP server component)

Since `claude-token-analyzer` bundles an MCP server, it can also be listed in MCP-specific directories:

| Directory | Size | Submission |
|---|---|---|
| **[Official MCP Registry](https://registry.modelcontextprotocol.io)** | Canonical registry | CLI tool: `mcp-publisher publish` with namespace auth |
| **[PulseMCP](https://pulsemcp.com/servers)** | 12,770+ servers | Submit at [pulsemcp.com/submit](https://pulsemcp.com/use-cases/submit) |
| **[MCP Market](https://mcpmarket.com/)** | Growing directory | Submit GitHub repo at [mcpmarket.com/submit](https://mcpmarket.com/submit) |
| **[mcpservers.org](https://mcpservers.org/)** | Curated collection | Free listing via "Submit" link; paid "Premium Submit" with badge |
| **[mcp.so](https://mcp.so)** | Large aggregator | Submit via site |
| **[mcpserverfinder.com](https://mcpserverfinder.com/)** | Search-focused | Submit via site |
| **[mcpcat.io](https://mcpcat.io/)** | Curated guides | Submit via site |
| **[Portkey MCP Servers](https://portkey.ai/mcp-servers)** | 138+ servers | Submission-based |
| **[Cline MCP Marketplace](https://github.com/cline/mcp-marketplace)** | Cline-specific | GitHub PR submission |

---

## 2. Curated "Awesome" Lists (GitHub)

These accept pull request submissions and are high-value for discovery:

| Repository | Stars/Focus | Submission |
|---|---|---|
| **[ccplugins/awesome-claude-code-plugins](https://github.com/ccplugins/awesome-claude-code-plugins)** | ~136 plugins listed; slash commands, subagents, MCP, hooks | PR to add listing |
| **[ComposioHQ/awesome-claude-plugins](https://github.com/ComposioHQ/awesome-claude-plugins)** | Organized by function (Integrations, Code Quality, etc.) | Fork + PR |
| **[hesreallyhim/awesome-claude-code](https://github.com/hesreallyhim/awesome-claude-code)** | Skills, hooks, commands, orchestrators, apps, plugins | PR |
| **[jmanhype/awesome-claude-code](https://github.com/jmanhype/awesome-claude-code)** | Plugins, MCP servers, editor integrations | PR |
| **[subinium/awesome-claude-code](https://github.com/subinium/awesome-claude-code)** | Tools, skills, plugins, MCP servers | PR |
| **[modelcontextprotocol/servers](https://github.com/modelcontextprotocol/servers)** | Official MCP servers repo | PR (high bar) |

---

## 3. Community Channels for Promotion

### 3.1 Discord
- **Official Claude Discord** ([discord.com/invite/6PPFFzqPDZ](https://discord.com/invite/6PPFFzqPDZ)): 75,670+ members. Has channels for sharing projects, plugins, and MCP servers.

### 3.2 Reddit
- **r/ClaudeAI** — Primary subreddit. Claude Code topics generate 4x more discussion volume than competing tools. Good for "I built this" posts with genuine utility demonstration.
- **r/MachineLearning**, **r/LocalLLaMA**, **r/programming** — Broader audiences for cross-posting.

### 3.3 Hacker News
- **"Show HN" posts** are actively used to launch MCP servers and Claude Code plugins. Multiple successful launches found in search results. Best posted Tuesday-Thursday mornings.

### 3.4 GitHub
- **anthropics/claude-code Discussions** — Official GitHub Discussions for Claude Code.
- **Issues/feature requests** referencing your tool in relevant repos.

### 3.5 Dev Blogs / Content Platforms
| Platform | Best For |
|---|---|
| **Dev.to** | Tutorials, how-to guides |
| **Medium** | Thought leadership, case studies |
| **Personal blog** | SEO, detailed walkthroughs |
| **Twitter/X** | Quick updates, threads, community engagement |

---

## 4. What Successful Projects Have Done

### 4.1 Case Study: ccusage (4.8k GitHub stars)

The closest competitor to `claude-token-analyzer` is [ccusage](https://github.com/ryoppippi/ccusage), a CLI tool for analyzing Claude Code usage from JSONL files. Its growth strategy (inferred):

1. **Solved a real pain point** — Claude Code cost tracking was opaque; ccusage made it visible.
2. **Got referenced in official docs** — Featured in Claude Code optimization guides and third-party blog posts.
3. **Organic word-of-mouth** — Described as "the Claude Code cost scorecard that went viral."
4. **Own website** — [ccusage.com](https://ccusage.com/) with polished landing page.
5. **Multiple blog mentions** — Referenced by Shipyard, ClaudeLog, and other Claude ecosystem blogs.

### 4.2 Common Patterns from Successful MCP/Plugin Projects

- **Show HN launch** — Multiple MCP servers have launched successfully on Hacker News.
- **Blog post + GitHub** — Most successful plugins have a detailed blog post explaining the "why" (e.g., "Building My First Claude Code Plugin" on alexop.dev, "The Deep Trilogy" on Medium).
- **Listed in multiple directories** — Top projects appear on 3-5 directories simultaneously.
- **Clear README with one-liner install** — The install command is front and center.
- **Polished documentation** — Documentation serves as the "primary sales tool" for developer tools.

---

## 5. Official MCP Registry Publishing (Technical Details)

To publish to the canonical MCP registry at `registry.modelcontextprotocol.io`:

1. **Install CLI:** `brew install mcp-publisher`
2. **Initialize:** `mcp-publisher init` (generates `server.json`)
3. **Configure namespace:** Use `io.github.li195111/claude-token-analyzer`
4. **Authenticate:** `mcp-publisher login github` (GitHub OAuth)
5. **Publish:** `mcp-publisher publish`
6. **Verify:** `curl "https://registry.modelcontextprotocol.io/v0/servers?search=claude-token-analyzer"`

Supports package (npm/PyPI/Docker), remote (SSE/HTTP), or hybrid deployment models.

Reference: [Publishing guide](https://modelcontextprotocol.info/tools/registry/publishing/)

---

## 6. Competitive Landscape

Other token/cost analysis tools for Claude Code:

| Tool | Type | Stars | Differentiator |
|---|---|---|---|
| **ccusage** | CLI | 4.8k | JSONL parsing, monthly reports, billing windows |
| **claude-code-usage-analyzer** | CLI | — | Uses ccusage + LiteLLM pricing |
| **Claude-Code-Usage-Monitor** | Terminal UI | — | Real-time monitoring, ML predictions |
| **claude-token-tracker** | Dashboard | — | Real-time dashboard, cost estimation |
| **token-optimizer-mcp** | MCP server | — | Token reduction via caching/compression |
| **Claude Usage Tracker** | Chrome ext. | — | Browser-based quota monitoring |

**CTA's differentiators** vs. competition: Plugin-native (not standalone CLI), MCP server architecture, anomaly detection with 6 statistical types, trend forecasting, integrated skills system.

---

## 7. Recommended Promotion Strategy (Prioritized)

### Tier 1 — High Impact, Do First

| Action | Expected Impact | Effort |
|---|---|---|
| Submit to **Anthropic official plugin directory** via submission form | Highest visibility (built into Claude Code UI) | Low |
| Submit to **Official MCP Registry** via `mcp-publisher` CLI | Canonical MCP discovery | Medium |
| PR to **awesome-claude-code-plugins** (ccplugins) | 136+ plugin catalog | Low |
| PR to **awesome-claude-plugins** (ComposioHQ) | Popular curated list | Low |
| PR to **awesome-claude-code** (hesreallyhim) | Comprehensive Claude Code list | Low |

### Tier 2 — Medium Impact, Do Next

| Action | Expected Impact | Effort |
|---|---|---|
| Submit to **PulseMCP** (12,770+ servers) | Large MCP audience | Low |
| Submit to **MCP Market** | Growing directory | Low |
| Submit to **mcpservers.org** | Curated, has premium option | Low |
| Submit to **buildwithclaude.com** marketplace | Community marketplace | Low |
| **Show HN post** with compelling title | Viral potential, technical audience | Medium |
| **Reddit r/ClaudeAI post** — "I built a Claude Code plugin for token analysis" | Direct target audience | Medium |

### Tier 3 — Sustained Marketing

| Action | Expected Impact | Effort |
|---|---|---|
| Write **blog post** (Dev.to or Medium) — tutorial-style | SEO, long-term discovery | High |
| Create **landing page / website** (like ccusage.com) | Professional presence, SEO | High |
| Post in **Claude Discord** #projects or #plugins channel | Community engagement | Low |
| **Twitter/X thread** with screenshots showing anomaly detection | Social proof | Medium |
| Cross-list in **general awesome MCP repos** (modelcontextprotocol/servers) | Broad MCP audience | Medium |

### Tier 4 — Long-term Growth

| Action | Expected Impact | Effort |
|---|---|---|
| Get referenced in **Claude Code documentation or blog posts** | Authority signal (like ccusage achieved) | Organic |
| **Video demo** (YouTube, Loom) showing real cost savings | Visual proof, shareable | High |
| Write **comparison post** vs. ccusage (CTA's unique advantages) | SEO, differentiation | Medium |
| Contribute to **Claude Code ecosystem** (issues, discussions) to build reputation | Community standing | Ongoing |

---

## 8. Key Insights

1. **The plugin marketplace is the #1 channel.** Getting listed on Anthropic's official marketplace (`claude-plugins-official`) means users can discover and install directly from within Claude Code. This should be the top priority.

2. **MCP directories are fragmented but plentiful.** There are 10+ directories that accept submissions. Submitting to all of them is low-effort, high-reward.

3. **Curated "awesome" lists drive GitHub discovery.** At least 5 active awesome lists accept PRs for Claude Code plugins.

4. **ccusage is the benchmark.** With 4.8k stars, it proves there is strong demand for Claude Code cost analysis tools. CTA's differentiators (plugin-native, anomaly detection, trend forecasting) should be emphasized.

5. **Content marketing works for dev tools.** Successful plugins have accompanying blog posts, tutorials, and Show HN launches. The "why" matters more than the "what" for developers.

6. **Documentation is marketing.** A polished README, clear install instructions, and good examples are the most effective sales tool for developer audiences.

7. **claudemarketplaces.com requires 500+ installs** to list a plugin, so focus on the official marketplace and awesome lists first to build install base.

---

## 9. Existing Web Presence of claude-token-analyzer

As of 2026-03-27, searching for "claude-token-analyzer" returns **no dedicated results**. The project does not yet appear in any directory, blog post, or curated list. This is a greenfield opportunity — all channels are available for first-time submissions.

The GitHub repository exists at `github.com/li195111/claude-token-analyzer` but has not been submitted to any external directory or marketplace yet.
