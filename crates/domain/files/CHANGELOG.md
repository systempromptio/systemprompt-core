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
- Workspace-aligned release. File storage surface unchanged.

## [0.9.2] - 2026-05-14

### Changed
- Normalize `CHANGELOG.md` format to align with workspace-wide maintainer style.

## [0.1.18] - 2026-03-27

### Changed
- Upgrade crate to the Rust 2024 edition.

## [0.1.0] - 2026-02-02

### Changed
- Promote crate to a stable `0.1.0` release aligned with the workspace baseline.

## [0.0.13] - 2026-01-27

### Changed
- Bump version for workspace consistency.

## [0.0.3] - 2026-01-22

### Added
- Add migration system infrastructure for schema rollout.

### Fixed
- Fix schema validation for VIEW-based schemas.

## [0.0.2] - 2026-01-22

### Added
- Distribute schema registration so each domain crate owns its SQL via the `Extension` trait.

### Changed
- Remove the centralised module loader path from `systemprompt-loader` in favour of per-crate ownership.

### Fixed
- Correct `include_str!` paths that previously pointed outside the crate directory.
- Ensure the crate compiles standalone when consumed from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release of `systemprompt-files`.
