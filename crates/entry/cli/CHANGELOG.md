# Changelog

## [0.0.3] - 2026-01-22

### Added
- `systemprompt infra db migrations status` command to show migration status for all extensions
- `systemprompt infra db migrations history <extension>` command to show migration history

### Changed
- Profile builders now include `extensions` configuration field

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
