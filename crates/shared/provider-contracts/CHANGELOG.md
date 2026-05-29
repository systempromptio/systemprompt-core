# Changelog

## [0.13.0] - 2026-05-28

### Changed

- Workspace version bump; no API changes in this crate.

## [0.12.0] - 2026-05-27

### Changed

- Workspace version bump; no API changes in this crate.

## [0.11.0] - 2026-05-20

### Changed
- Workspace-aligned release. Provider trait definitions track the tenancy strip in `domain/ai` and `domain/oauth`: provider call sites no longer thread a runtime `tenant_id`.

## [0.9.2] - 2026-05-14

### Changed
- Normalized changelog format to maintainer style with explicit categories.

## [0.1.0] - 2026-02-02

### Changed
- Aligned to workspace 0.1.0 release.

## [0.0.13] - 2026-01-27

### Changed
- Workspace version bump.

## [0.0.3] - 2026-01-22

### Added
- Migration system infrastructure.

### Fixed
- Schema validation now handles VIEW-based schemas.

## [0.0.2] - 2026-01-22

### Changed
- Each domain crate now owns its SQL schemas via the `Extension` trait.

### Removed
- Centralized module loaders from `systemprompt-loader`.

### Fixed
- Corrected `include_str!` paths that pointed outside the crate directory.
- Ensured the crate compiles standalone when downloaded from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
