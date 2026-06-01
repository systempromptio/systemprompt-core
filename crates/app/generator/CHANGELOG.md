# Changelog

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
- Workspace-aligned release. Generator surface unchanged.

## [0.9.2] - 2026-05-14

### Changed
- Normalize changelog formatting to maintainer style.

## [0.1.3] - 2026-03-20

### Fixed
- Drop stale tests referencing removed `BuildError` variants.

## [0.1.2] - 2026-03-05

### Added
- Add `futures` dependency for stream-based concurrency in the prerender pipeline.

### Changed
- Render content items concurrently with `buffer_unordered(8)`.
- Enrich content concurrently with `buffered(8)` per source.
- Process sources concurrently with `buffer_unordered(2)`.
- Replace regex-based heading-ID injection with a string search in TOC generation.

### Removed
- Drop the `regex` dependency.

## [0.1.1] - 2026-02-03

### Added
- Priority-based deduplication for page prerenderers so lower-priority prerenderers skip already-rendered page types.
- Priority-based deduplication for component renderers so lower-priority components skip already-rendered variables.

## [0.1.0] - 2026-02-02

### Changed
- First stable release; aligned with workspace 0.1.0.

## [0.0.14] - 2026-01-27

### Added
- Generate a Table of Contents for documentation pages via a new `toc` module.
- Extract headings from the comrak AST and inject anchor IDs automatically.
- Emit TOC HTML with stylable classes (`.toc-list`, `.toc-item`, `.toc-level-N`, `.toc-link`).
- Disambiguate duplicate heading slugs with numeric suffixes.

## [0.0.13] - 2026-01-27

### Changed
- Bump version for workspace consistency.

## [0.0.3] - 2026-01-22

### Added
- Add migration-system infrastructure.

### Fixed
- Validate schemas backed by SQL VIEWs.

## [0.0.2] - 2026-01-22

### Changed
- Move schema registration to each domain crate via the `Extension` trait.
- Drop centralised module loaders from `systemprompt-loader`.

### Fixed
- Correct `include_str!` paths that pointed outside the crate directory.
- Ensure the crate compiles standalone when fetched from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
