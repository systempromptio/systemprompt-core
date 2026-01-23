# Changelog

## [0.0.3] - 2026-01-22

### Fixed
- Fix schema validation for VIEW-based schemas
- Add migration system infrastructure

## [0.0.3] - 2026-01-22

### Added
- `extension_migrations` table for tracking applied migrations
- `MigrationService` for running and tracking extension migrations
- `MigrationStatus`, `MigrationResult`, `AppliedMigration` types
- `install_extension_schemas_with_config()` function supporting disabled extensions
- Database extension marked as required (`is_required() -> true`)

### Changed
- Schema installation now runs pending migrations after base schema creation
- Migrations are tracked with version, name, and checksum for integrity validation

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
