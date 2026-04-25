# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Removed the pre-download `HEAD` request for uncached objects; missing artifacts now skip on `404` from the `GET`.
- Buffered archive writes and avoid repeated output directory canonicalization during extraction.
- Routed CLI `--objects` downloads through the shared library pipeline.

### Removed

- Removed optional S3 streaming support to keep `ycbust` focused on local YCB acquisition.
- Removed optional blocking wrappers; callers should use the async API or the CLI.
- Removed the deprecated non-standard `Ten` subset.
- Removed unused `YcbError::Integrity` and `YcbError::Other` variants.

## [0.4.1](https://github.com/Agentic-Insights/ycbust/compare/v0.4.0...v0.4.1) - 2026-04-23

### Other

- mirror bevy-sensor contract suite as upstream guard ([#31](https://github.com/Agentic-Insights/ycbust/pull/31))
- tidy blocking-snippet in README; lock README snippets behind a compile test ([#29](https://github.com/Agentic-Insights/ycbust/pull/29))

## [0.4.0](https://github.com/Agentic-Insights/ycbust/compare/v0.3.3...v0.4.0) - 2026-04-23

### Added

- [**breaking**] typed errors, parallel downloads, blocking wrappers, integrity check (v0.4.0) ([#26](https://github.com/Agentic-Insights/ycbust/pull/26))

### Fixed

- v0.4.0 pre-release review tweaks ([#28](https://github.com/Agentic-Insights/ycbust/pull/28))

## [0.3.3](https://github.com/Agentic-Insights/ycbust/compare/v0.3.2...v0.3.3) - 2026-04-23

### Added

- public download_objects + GOOGLE_16K path consts ([#24](https://github.com/Agentic-Insights/ycbust/pull/24))

## [0.3.2](https://github.com/Agentic-Insights/ycbust/compare/v0.3.1...v0.3.2) - 2026-04-23

### Fixed

- s3 subset handling, windows temp dir default, full-features CI ([#13](https://github.com/Agentic-Insights/ycbust/pull/13))

### Other

- refresh README for v0.3 subcommands and portable defaults ([#14](https://github.com/Agentic-Insights/ycbust/pull/14))

## [0.3.1](https://github.com/Agentic-Insights/ycbust/compare/v0.3.0...v0.3.1) - 2026-03-25

### Other

- make object path doctest platform agnostic
- codify downstream ownership and release cadence
- release v0.2.6

## [0.3.0](https://github.com/Agentic-Insights/ycbust/compare/v0.2.5...v0.3.0) - 2026-02-27

### Added

- [**breaking**] v0.3.0 — TBP standard objects, subcommands, validate + list ([#10](https://github.com/Agentic-Insights/ycbust/pull/10))

## [0.2.5](https://github.com/Agentic-Insights/ycbust/compare/v0.2.4...v0.2.5) - 2025-12-25

### Changed

- Convert license from MIT to Apache 2.0 for patent grant protection
- Migrate repository to Agentic-Insights organization
- Reorganize justfile with command groupings for better developer experience

## [0.2.4](https://github.com/Agentic-Insights/ycbust/compare/v0.2.3...v0.2.4) - 2025-12-14

### Other

- add Development section to README
- add justfile for better DevEx
- add crate link, release badges, and installation instructions

## [0.2.3](https://github.com/Agentic-Insights/ycbust/compare/v0.2.2...v0.2.3) - 2025-12-13

### Added

- make ycbust importable as a library

## [0.2.2](https://github.com/Agentic-Insights/ycbust/compare/v0.2.1...v0.2.2) - 2025-12-13

### Fixed

- update YCB S3 URL and fix tar extraction

## [0.2.1](https://github.com/Agentic-Insights/ycbust/compare/v0.2.0...v0.2.1) - 2025-11-30

### Added

- add security fixes and test coverage

### Fixed

- address code review findings
