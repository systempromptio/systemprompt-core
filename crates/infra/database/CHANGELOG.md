# Changelog

## [0.1.9] - 2026-02-05

### Fixed
- Fix `MigrationService` to ensure `extension_migrations` table exists before querying
  - Adds `ensure_migrations_table_exists()` method using `CREATE TABLE IF NOT EXISTS`
  - Called in `run_pending_migrations()` and `get_migration_status()` before table queries
  - Prevents "relation does not exist" errors on fresh database initialization
  - Handles edge cases: disabled database extension, direct API usage, registration failures

## [0.1.0] - 2026-02-02

### Changed
- First stable release milestone
- All crates now at consistent 0.1.0 version

## [0.0.14] - 2026-01-27

### Changed
- Add `include` directive to Cargo.toml for SQLx offline mode support
- Published crates now include `.sqlx/` query cache for offline compilation

## [0.0.13] - 2026-01-27

### Changed
- Version bump for workspace consistency

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
