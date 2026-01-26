# Changelog

## [0.0.11] - 2026-01-26

### Changed
- Improve `SessionStore` management and key handling
- Refactor CLI session store for better reliability

## [0.0.4] - 2026-01-23

### Added
- `cancel_subscription` method to CloudApiClient for subscription cancellation
- `update_from_tenant_info` method to preserve credentials during tenant sync

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
