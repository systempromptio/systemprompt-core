# Changelog

## [0.0.11] - 2026-01-26

### Changed
- Pass JWT provider to OAuth validation in A2A request handler

### Fixed
- Filter null JSON values in `ArtifactBuilder::build_artifacts()` to prevent errors when tools return `Some(Value::Null)`

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
