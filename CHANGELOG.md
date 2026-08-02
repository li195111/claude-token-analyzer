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

### Changed

- Installation now uses marketplace registration followed by the qualified
  `claude-token-analyzer@claude-token-analyzer` plugin id.
- Runtime binaries are installed under versioned filenames and downloaded from
  the release tag matching the plugin manifest.
- Human-readable output templates are localizable while technical identifiers
  remain English.

### Fixed

- Removed the invalid marketplace `metadata.homepage` field and duplicate
  marketplace version declaration.
- Removed the redundant SessionStart installer path.
- Restored locked Cargo/clippy verification and synchronized plugin, Cargo, and
  lockfile versions.

### Integrity boundary

- Downloaded binaries are verified against the same release's `SHA256SUMS`
  before atomic installation. This proves transport integrity and release-asset
  identity; it does not protect against compromise of the GitHub publisher.
