# Changelog

## [0.0.13] - 2026-01-27

### Changed
- Use `Self::` instead of type name in Part enum match arms for clippy compliance

## [0.0.11] - 2026-01-26

### Changed
- `ToolResponse::to_json()` now returns `Result<JsonValue, serde_json::Error>` instead of silently returning `Null` on error
- `Artifact::to_json_value()` trait method now returns `Result<JsonValue, serde_json::Error>` instead of silently returning `Null` on error

## [0.0.7] - 2026-01-23

### Changed
- `RotateCredentialsResponse` now returns `internal_database_url` and `external_database_url` instead of single `database_url` field

## [0.0.4] - 2026-01-23

### Added
- `tenant_subscription_cancel` API path for subscription cancellation
- `ExtensionsConfig` struct for profile-based extension enable/disable configuration
- `extensions` field in `Profile` struct
- `is_masked_database_url` helper to detect masked credentials

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
