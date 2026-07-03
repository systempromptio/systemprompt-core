# Changelog

## [0.19.0] - 2026-07-02

### Breaking

- The minimum supported Rust version is 1.94.
- SQLx is upgraded to 0.9.
- The `ContentProvider` implementation follows the renamed trait methods: `find_content*` (was `get_content*`).

### Changed

- Connection-pool acquisition failures are classified as repository errors rather than surfacing as a generic failure.

## [0.16.0] - 2026-06-22

### Breaking

- The minimum supported Rust version is 1.88.

### Fixed

- Frontmatter parsing is line-anchored: the opening and closing `---` must each be a full line, so `---` sequences inside the document body are no longer mistaken for delimiters.

## [0.14.0] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.13.1] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.13.0] - 2026-05-28

### Changed

- Workspace version bump; no API changes in this crate.

## [0.12.0] - 2026-05-27

### Changed

- Workspace version bump; no API changes in this crate.

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
