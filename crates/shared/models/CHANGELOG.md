# Changelog

## [0.0.3] - 2026-01-23

### Added
- `tenant_subscription_cancel` API path for subscription cancellation
- `ExtensionsConfig` struct for profile-based extension enable/disable configuration
- `extensions` field in `Profile` struct

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
