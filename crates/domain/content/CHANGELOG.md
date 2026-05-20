# Changelog

## [0.11.0] - 2026-05-20

### Changed
- Workspace-aligned release. Front-matter `format!` call sites cleaned up of redundant references; no public API change.

## [0.9.2] - 2026-05-14

### Changed
- Normalize changelog formatting to maintainer style.

## [0.1.18] - 2026-03-27

### Added
- `public` field on `CreateContentParams` and `ContentMetadata` for content visibility control.

### Changed
- Upgrade to Rust 2024 edition.

## [0.1.10] - 2026-02-08

### Added
- `ContentRouting` trait implementation for `ContentConfigValidated`.
- `resolve_slug` method for extracting content slugs from URL patterns.
- `determine_source` method for identifying content sources from paths.

## [0.1.1] - 2026-02-03

### Changed
- Replace `unwrap_or_default` with explicit `map_or_else` patterns in list renderers.

## [0.1.0] - 2026-02-02

### Changed
- First stable release aligning all workspace crates at 0.1.0.

## [0.0.13] - 2026-01-27

### Changed
- Version bump for workspace consistency.

## [0.0.12] - 2026-01-27

### Added
- Expose `UpdateContentParams` builder methods for CLI edit command integration.

## [0.0.11] - 2026-01-26

### Removed
- `allowed_content_types` parameter from scanner and validation functions.

### Changed
- Simplify content ingestion API by removing content type filtering at scan time.

## [0.0.3] - 2026-01-22

### Added
- Migration system infrastructure.

### Fixed
- Schema validation for VIEW-based schemas.

## [0.0.2] - 2026-01-22

### Changed
- Implement distributed schema registration so each domain crate owns its SQL schemas via the `Extension` trait.
- Remove centralized module loaders from `systemprompt-loader`.

### Fixed
- Correct `include_str!` paths that pointed outside the crate directory.
- Ensure the crate compiles standalone when downloaded from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
