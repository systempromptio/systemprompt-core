# Changelog

All notable changes to `systemprompt-database` are documented here. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.10.0] - 2026-05-12

### Breaking
- **Breaking:** `SqlExecutor::parse_sql_statements` now returns `DatabaseResult<Vec<String>>` instead of `Vec<String>`. Migrate by propagating the new `Result` and handling `RepositoryError::Internal` for unparseable SQL.
- **Breaking:** Removed internal helpers `SqlExecutor::should_skip_line` and `SqlExecutor::is_statement_complete`. Migrate by relying on `sqlparser`-driven parsing exposed through `parse_sql_statements`.

### Changed
- Replaced the line-scanner statement splitter with `sqlparser` against `PostgreSqlDialect`, with correct handling of named dollar-quoted bodies (`$tag$ … $tag$`) and apostrophe-quoted function bodies.

## [0.9.2] - 2026-05-12

### Fixed
- `SqlExecutor::parse_sql_statements` no longer treats `CREATE TRIGGER` as opening a plpgsql function body, restoring schema install on a clean database.

## [0.1.18] - 2026-03-27

### Added
- `Database::read_pool` and `Database::read_pool_arc` accessors for explicit read-only pool access.

### Changed
- Upgraded to the Rust 2024 edition.

### Fixed
- Routed `Database::pool` and write operations through the configured write provider when one is available.

## [0.1.10] - 2026-02-19

### Breaking
- **Breaking:** Removed the `server_type` field from `CreateServiceInput`. Migrate by setting `server_type` on `McpServerConfig` instead of on service registration.

## [0.1.9] - 2026-02-05

### Fixed
- `MigrationService` now ensures the `extension_migrations` table exists before `run_pending_migrations` and `get_migration_status` query it, preventing "relation does not exist" errors on fresh databases.

## [0.1.0] - 2026-02-02

### Changed
- First stable release; aligned to the workspace `0.1.0` version baseline.

## [0.0.14] - 2026-01-27

### Changed
- Published crate now ships the `.sqlx/` query cache for SQLx offline compilation.

## [0.0.13] - 2026-01-27

### Changed
- Workspace version alignment release.

## [0.0.4] - 2026-01-22

### Fixed
- Schema validation now accepts view-based schemas.

## [0.0.3] - 2026-01-22

### Added
- `extension_migrations` table and `MigrationService` for running and tracking extension migrations.
- `MigrationStatus`, `MigrationResult`, and `AppliedMigration` types.
- `install_extension_schemas_with_config` for installing schemas with disabled extensions skipped.

### Changed
- Schema installation now runs pending migrations after base schema creation, tracking version, name, and checksum for integrity validation.
- Database extension reports `Extension::is_required() == true`.

## [0.0.2] - 2026-01-22

### Added
- Distributed schema registration: each domain crate owns its SQL schemas via the `Extension` trait.

### Removed
- Centralized module loaders previously hosted in `systemprompt-loader`. Migrate by registering schemas through your crate's `Extension` implementation.

### Fixed
- `include_str!` paths now resolve inside the crate root, allowing the crate to compile standalone from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
