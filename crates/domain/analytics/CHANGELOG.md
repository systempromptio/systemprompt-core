# Changelog

## [0.0.13] - 2026-01-27

### Changed
- Use `is_none_or` instead of `map_or` in bot detection for clarity

## [0.0.11] - 2026-01-26

### Added
- Fan out engagement metrics on `PageExit` analytics events
- `fan_out_engagement` for batch analytics processing

### Fixed
- Fix clippy errors in repository modules

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
