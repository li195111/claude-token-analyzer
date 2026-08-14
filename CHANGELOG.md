# Changelog

All notable changes to this project are documented here. Release dates are
filled only when the corresponding tag is published.

## [0.3.0] - Unreleased

### Added

- Native Claude Code `userConfig.output_language` support for `auto`, `en`, and
  `zh-TW` across all seven CTA skills.
- MCP initialize identity and a stdio smoke that verifies version `0.3.0` and
  the eight-tool inventory.
- Release `SHA256SUMS` generation and installer integration coverage.
- `CTA_LOCAL_BINARY` development override so a locally built MCP server runs
  through the standard plugin entrypoint.

### Changed

- Installation now uses marketplace registration followed by the qualified
  `claude-token-analyzer@claude-token-analyzer` plugin id.
- Runtime binaries are installed under versioned filenames and downloaded from
  the release tag matching the plugin manifest.
- Human-readable output templates are localizable while technical identifiers
  remain English.
- The output-language contract states explicitly that a configured `en` or
  `zh-TW` value overrides the user's message language, and every skill
  self-checks the response language before sending.

### Fixed

- Removed the invalid marketplace `metadata.homepage` field and duplicate
  marketplace version declaration.
- Removed the redundant SessionStart installer path.
- Restored locked Cargo/clippy verification and synchronized plugin, Cargo, and
  lockfile versions.
- Installer downloads are time-bounded with bounded retries, stale installer
  temp files are swept, and the tag-time release gate now runs the plugin
  validate and marketplace install contracts.
- The pricing table covers the Claude 5 family and all current 4.x models;
  stale Opus 4.6 and Haiku 4.5 rates are corrected to current list prices, and
  dated legacy model IDs resolve via aliases instead of silently falling back
  to Sonnet-equivalent defaults.

### Integrity boundary

- Downloaded binaries are verified against the same release's `SHA256SUMS`
  before atomic installation. This proves transport integrity and release-asset
  identity; it does not protect against compromise of the GitHub publisher.
- Cached binaries are re-verified on every launch against a checksum record
  written at install time; a corrupted or unrecorded cache entry is reinstalled
  instead of being trusted.
