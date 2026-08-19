# Changelog

## [0.32.0] - 2026-08-19

### Breaking

- **Breaking:** The migration-squash machinery is removed: `MigrationService::squash`, `SquashPlan`, and the filesystem `squash_baseline` module (`SquashBaselineService`, `SquashBaselineError`). The declarative schema is the baseline; migrate by deleting squash workflows.

### Added

- Fresh installs are stamped instead of migrated: `MigrationService::assess_freshness` (`FreshnessCheck`) detects a database with no tracking history and none of the extension's tables, and `stamp_all_migrations` records every defined migration without executing its SQL — the structural/dependent DDL alone produces the target schema.
- `MigrationService::reconcile_drift` rewrites the stored checksum of a drifted migration without executing any SQL, for migrations whose schema effects are already in place.

### Changed

- `MigrationService::repair_drift` re-executes each drifted migration and rewrites its stored checksum inside a single transaction instead of deleting the tracking row first; a failed re-apply now rolls back to the tracked, drifted state instead of leaving the migration untracked and re-run on every boot. `RepairResult::migrations_run` counts only genuinely pending migrations.
- The checksum-drift error recommends `migrate-repair --reconcile-only --apply` for already-applied edited migrations, with plain `--apply` reserved for deliberately re-executing SQL.
- Migration application, revert, and repair record their tracking-table write in the same transaction as the migration statements, and repair holds the bootstrap advisory lock.

## [0.27.0] - 2026-07-29

### Breaking

- **Breaking:** `CleanupRepository::delete_orphaned_mcp_executions` is removed. `mcp_tool_executions.context_id` carries no foreign key, so a context id absent from `user_contexts` is a normal state — the sweep hard-deleted same-day governance audit rows the moment their context disappeared. Tool executions outlive their context; consumers aggregate by `user_id`.

### Fixed

- `Database::get_info` skips a table dropped between the table-list query and its per-table row count, so introspection running concurrently with a migration no longer fails with `42P01`.

### Added

- `CleanupRepository::count_orphaned_logs` reports the would-delete count for observe-mode (`enforce: false`) runs of `database_cleanup`.
- Connection-scope seam for pooled multi-tenancy row-level security: `scope::ConnectionScopeProvider` (registered with `register_scope_provider!`) translates a `RequestScope` into transaction-local custom GUCs, applied via parameter-bound `set_config($1, $2, true)` on the new `begin_scoped` / `with_scoped_transaction{,_raw}` / `Database::begin_scoped` APIs. Settings die at COMMIT/ROLLBACK, so pooled connections return clean. Strictly opt-in: with no registered provider the scoped APIs are a plain `pool.begin()`, and the existing pool and transaction surfaces are untouched.

## [0.24.0] - 2026-07-26

### Fixed

- The database extension declares its shared trigger functions with `SchemaDefinition::sql_only`, so it no longer claims to own a table named `functions` that its SQL does not create.

### Removed

- **Breaking:** `DatabaseAdminService::list_expected_tables` is removed. Migrate by reconciling against the tables declared by registered extensions, as `infra db doctor` does.

## [0.21.1] - 2026-07-17

### Changed
- Source files now carry a Business Source License 1.1 header referencing <https://systemprompt.io>.

### Fixed
- Multi-statement seed bodies now apply correctly; each parsed statement executes individually within the seed's transaction instead of failing as a single multi-command prepared statement.

## [0.19.0] - 2026-07-02

### Breaking

- SQLx is upgraded to 0.9; the pool and row types this crate re-exports now come from sqlx 0.9, and dynamic SQL passed through the raw-execution seams is wrapped with `sqlx::AssertSqlSafe`. Consumers must build against sqlx 0.9.
- The minimum supported Rust version is 1.94.

## [0.16.1] - 2026-06-22

### Added

- `CleanupRepository::delete_expired_id_jag_replays` prunes expired ID-JAG replay rows.

## [0.16.0] - 2026-06-22

### Breaking

- The minimum supported Rust version is 1.88.

### Added

- The connection pool is operator-configurable through the profile's `database.pool` block (`max_connections`, `min_connections`, `acquire_timeout`, `idle_timeout`, `max_lifetime`); values are validated at bootstrap.

## [0.14.0] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

All notable changes to `systemprompt-database` are documented here. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.13.1] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.13.0] - 2026-05-28

### Changed

- Workspace version bump; no API changes in this crate.

## [0.12.0] - 2026-05-27

### Changed

- Workspace version bump; no API changes in this crate.

## [0.11.0] - 2026-05-20

### Added
- Read-replica routing. `DbPool` honours an optional read-replica URL and routes read-only queries to it; writes continue to land on the primary.
- `infra db migrate-repair --apply` reconciles migration checksum drift in place by retiring the drifted bookkeeping rows and re-applying the affected migrations idempotently.

### Changed
- The migration-runner checksum-drift error message now points operators at `infra db migrate-repair --apply`, replacing the previous `--allow-checksum-drift` hint which suppressed the symptom without resolving it.

## [0.10.2] - 2026-05-15

### Added
- `resilience` module: domain-agnostic resilience primitives (`ResilienceGuard`, `CircuitBreaker`, `Bulkhead`, `retry_async`, `guarded_stream`) for wrapping outbound calls, generic over a caller-supplied error type and classifier.
- Boot-time table-ownership validation: schema installation rejects two extensions creating the same table, and a `cross_extension_tables()` entry no other extension creates, before any DDL runs.

### Changed
- `connect_with_retry` and `with_transaction_retry` now run their backoff on `resilience::retry::retry_async` instead of a hand-written loop. Retry behaviour (attempt counts, delays, error classification) is unchanged.
- The schema-install statement classifier matches every `pg_query` DDL node variant explicitly; an unrecognised node fails installation instead of being silently treated as a dependent statement.
- Seed linting rejects a non-idempotent `INSERT` (one with no `ON CONFLICT` clause).
- Required-column validation is schema-qualified rather than assuming the `public` schema.

### Fixed
- `DatabaseExtension` declares priority `0` so its shared SQL helpers and the `extension_migrations` table install before every other extension; without it, install order tie-broke alphabetically and could place a dependent extension first.

## [0.10.0] - 2026-05-12

### Breaking
- **Breaking:** `SqlExecutor::parse_sql_statements` now returns `DatabaseResult<Vec<String>>` instead of `Vec<String>`. Migrate by propagating the new `Result` and handling `RepositoryError::Internal` for unparseable SQL.
- **Breaking:** Removed internal helpers `SqlExecutor::should_skip_line` and `SqlExecutor::is_statement_complete`. Migrate by relying on the statement splitting exposed through `parse_sql_statements`.

### Changed
- Replaced the line-scanner statement splitter with a hand-rolled byte-state-machine splitter that splits on top-level `;` while ignoring semicolons inside single-quoted strings, dollar-quoted bodies (`$$ … $$` and `$tag$ … $tag$`), `--` line comments, and nested `/* … */` block comments. The splitter preserves the original statement text verbatim — a parse-and-reprint approach drops syntactic detail such as the empty parameter list on `CREATE FUNCTION foo()`, which PostgreSQL then rejects.

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
