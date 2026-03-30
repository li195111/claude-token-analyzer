# Awesome List PR Entries

Ready-to-copy entries for 5 awesome list repositories. Each entry matches the target repo's verified format.

---

## B1 — hesreallyhim/awesome-claude-code

**Section:** Tooling > Usage Monitors
**Format:** `- [Name](url) by [Author](url) - description`

```
- [claude-token-analyzer](https://github.com/li195111/claude-token-analyzer) by [li195111](https://github.com/li195111) - Diagnoses token waste in Claude Code sessions with 6 anomaly types, cost audit, and trend forecasting. Fully local, plugin-native.
```

---

## B2 — jmanhype/awesome-claude-code

**Format:** Markdown table `| Name | Maintainer | Description |`

```
| [claude-token-analyzer](https://github.com/li195111/claude-token-analyzer) | li195111 | Diagnoses token waste in Claude Code sessions: 6 anomaly types, cost audit, trend forecasting. Fully local, plugin-native. |
```

---

## B3 — subinium/awesome-claude-code

**Format:** Table with stars badge

```
| [li195111/claude-token-analyzer](https://github.com/li195111/claude-token-analyzer) | ![stars](https://img.shields.io/github/stars/li195111/claude-token-analyzer?style=flat-square&logo=github) | Diagnoses token waste in Claude Code sessions with 6 anomaly types, cost audit, and trend forecasting |
```

---

## B4 — ccplugins/awesome-claude-code-plugins

**Action:** Fork repo → create `plugins/claude-token-analyzer/` subdirectory → add README.md inside with plugin description → add link in main README under Data Analytics section.

**Main README entry:**
```
- [claude-token-analyzer](plugins/claude-token-analyzer/) - Diagnoses token waste in Claude Code sessions with 6 anomaly types and severity scoring. Fully local.
```

**plugins/claude-token-analyzer/README.md content:**
```markdown
# claude-token-analyzer

Diagnoses token waste in Claude Code sessions. Detects 6 types of statistical anomalies with severity scoring. Fully local — parses ~/.claude JSONL files into SQLite, nothing leaves your machine.

- **Install:** `claude plugin install claude-token-analyzer`
- **GitHub:** https://github.com/li195111/claude-token-analyzer
- **License:** MIT
```

---

## B5 — ComposioHQ/awesome-claude-plugins

**Category:** Data Analytics
**Action:** Submit full plugin reference with README entry.

**README entry:**
```
**[claude-token-analyzer](https://github.com/li195111/claude-token-analyzer)** - Diagnoses token waste in Claude Code sessions with 6 anomaly types and severity scoring. Fully local — one command install.
```

---

## PR Description Template

Use this as the PR body for all 5 submissions:

```
## New Plugin: claude-token-analyzer

Diagnoses token waste in Claude Code sessions with 6 anomaly types, cost audit, and trend forecasting.

- **Install:** `claude plugin install claude-token-analyzer`
- **Fully local** — parses ~/.claude JSONL files into SQLite, nothing leaves your machine
- **MIT licensed**

GitHub: https://github.com/li195111/claude-token-analyzer
```
