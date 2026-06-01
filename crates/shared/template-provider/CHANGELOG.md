# Changelog

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
- Workspace-aligned release. Template provider trait surface unchanged.

## [0.9.2] - 2026-05-14

### Changed
- Normalized changelog formatting to maintainer style.

## [0.1.0] - 2026-02-02

### Changed
- First stable release milestone aligning all workspace crates at 0.1.0.

## [0.0.13] - 2026-01-27

### Changed
- Bumped version for workspace consistency.

## [0.0.3] - 2026-01-22

### Added
- Migration system infrastructure.

### Fixed
- Schema validation for VIEW-based schemas.

## [0.0.2] - 2026-01-22

### Changed
- Adopted distributed schema registration so each domain crate owns its SQL schemas via the `Extension` trait.
- Removed centralized module loaders from `systemprompt-loader`.

### Fixed
- Corrected `include_str!` paths that pointed outside the crate directory.
- Ensured the crate compiles standalone when downloaded from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
