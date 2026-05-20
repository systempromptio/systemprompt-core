# Changelog

## [0.11.0] - 2026-05-20

### Changed
- Workspace-aligned release. Extension trait surface unchanged; downstream crates continue to register via `register_extension!` and `extension_migrations!()`.

## [0.9.2] - 2026-05-14

### Breaking
- Removed `Extension::migration_weight()` and `SchemaExtensionTyped::migration_weight()`; extension ordering is the dependency graph only.
- Removed `Extension::owned_tables()`; an extension's owned tables are derived from the `CREATE TABLE` statements in its `schemas()`.
- Removed `LoaderError::InvalidDependencyOrdering`.

### Added
- `LoaderError::DuplicateTableOwner`, `CrossExtensionTableNotOwned`, and `SeedInsertNotIdempotent`.
- `SchemaDefinition::with_schema` and `schema_name()` for non-`public` schema-qualified tables.

### Changed
- Align crate version with the `systemprompt-core` workspace release cadence.

## [0.1.21] - 2026-04-02

### Changed
- Move `RESERVED_PATHS` to the `registry` module and re-export from `typed_registry`.

## [0.1.18] - 2026-03-27

### Changed
- Upgrade to Rust 2024 edition.
- Split `lib.rs` and `registry.rs` into focused submodules.

## [0.1.12] - 2026-02-10

### Added
- `AssetType::Html` variant for declaring HTML assets.
- `AssetDefinition::html()` convenience constructor.

## [0.1.10] - 2026-02-06

### Added
- `SiteAuthConfig` type for declaring site-wide authentication requirements.
- `Extension::site_auth()` method, defaulting to `None`.
- `Extension::has_site_auth()` predicate method.
- `SiteAuthConfig` re-exported from the prelude.

## [0.1.0] - 2026-02-02

### Changed
- First stable release.

## [0.0.13] - 2026-01-27

### Changed
- Version bump for workspace consistency.

## [0.0.3] - 2026-01-22

### Added
- `Migration` struct for versioned extension migrations.
- `Extension::is_required()` method to mark core extensions as non-disableable.
- `Extension::migrations()` method for declaring versioned migrations.
- `Extension::has_migrations()` helper.
- `LoaderError::MigrationFailed` variant.
- `enabled_extensions()`, `enabled_schema_extensions()`, `enabled_api_extensions()`, and `enabled_job_extensions()` filters on the registry.

### Fixed
- Schema validation for VIEW-based schemas.

## [0.0.2] - 2026-01-22

### Changed
- Each domain crate now owns its SQL schemas via the `Extension` trait, replacing the centralized loader in `systemprompt-loader`.

### Fixed
- `include_str!` paths that pointed outside the crate directory.
- Standalone compilation when consumed from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
