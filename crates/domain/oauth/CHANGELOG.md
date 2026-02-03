# Changelog

## [0.1.2] - 2026-02-03

### Changed
- Regenerated SQLx offline query cache

## [0.1.1] - 2026-02-03

### Added
- `SessionCreationError` typed error enum for session creation failures

### Fixed
- Validate user existence before creating authenticated session to prevent FK constraint violations

## [0.1.0] - 2026-02-02

### Changed
- First stable release milestone
- All crates now at consistent 0.1.0 version

## [0.0.13] - 2026-01-27

### Changed
- Version bump for workspace consistency

## [0.0.11] - 2026-01-26

### Added
- Add `fingerprint_hash` field to `AnonymousSessionInfo` for session tracking

## [0.0.3] - 2026-01-22

### Fixed
- Fix schema validation for VIEW-based schemas
- Add migration system infrastructure

## [0.0.2] - 2026-01-22

### Changed
- Implement distributed schema registration pattern
- Each domain crate now owns its SQL schemas via Extension trait
- Remove centralized module loaders from systemprompt-loader

### Fixed
- Fix `include_str!` paths that pointed outside crate directory
- Ensure crate compiles standalone when downloaded from crates.io

## [0.0.1] - 2026-01-21

- Initial release
