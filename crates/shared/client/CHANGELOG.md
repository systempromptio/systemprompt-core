# Changelog

## [0.1.23] - 2026-04-14

### Fixed
- `SystempromptClient::send_message` now uses the A2A v1.0.0 method name `SendMessage` instead of the legacy `message/send`, which the server rejects after the v0.3.0 → v1.0.0 migration

## [0.1.18] - 2026-03-27

### Changed
- Upgrade to Rust 2024 edition

## [0.1.0] - 2026-02-02

### Changed
- First stable release milestone
- All crates now at consistent 0.1.0 version

## [0.0.13] - 2026-01-27

### Changed
- Version bump for workspace consistency

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
