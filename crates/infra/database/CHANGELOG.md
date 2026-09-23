# Changelog

## [0.60.0] - 2026-09-23

### Fixed

- `DatabaseAdminService::list_tables` and `list_tables_counted` tolerate a relation dropped between the catalog read and its size or row count. Every boot re-creates the analytics `report_*` views, so `infra db size` and `infra db tables` failed with `relation … does not exist` whenever they ran beside a starting instance; the size now resolves through `to_regclass` and the count skips SQLSTATE 42P01, as `Database::get_info` already did.

## [0.59.0] - 2026-09-22

### Added

- Migration cost budgets: a migration declaring `-- @cost: rows=<n> measured=<duration> triggers=<suspended|live>` runs under a `statement_timeout` derived from what its author measured (ten times it, with a floor), and one declaring nothing runs under 5 minutes; every migration also runs under a 10 s `lock_timeout`. Each statement's elapsed time is logged past 5 s, with the migration's total on completion, and `SYSTEMPROMPT_MIGRATION_STATEMENT_TIMEOUT_SECS=0` disables the bound for an attended one-off. Migrations are awaited before the HTTP listener binds, so an unbounded one is not a slow boot but an instance that never opens its port — a production instance spent 27 minutes there on a 3,644-row UPDATE.
- `audit_migration_cost` (pg_query AST, beside `check_migration_references`) warns at boot for a migration that rewrites a hot table without declaring a cost, and is a hard failure in the integration suite against a static baseline.
- Undeclared cross-extension writes are refused: a migration that `ALTER`s a table its extension neither creates in `schemas()` nor declares in `Extension::cross_extension_tables()` fails with `CrossExtensionAlterUndeclared`, a declaration naming a table no other loaded extension creates fails with `CrossExtensionTableNotOwned`, and a table two extensions both create fails with `DuplicateTableOwner`.
- `audit_schema_residue` / `SchemaResidue`: live tables no loaded extension declares, and migration ledgers of extensions that no longer exist. Boot warns; `infra db doctor` exits non-zero.
- Retirement migrations are executed as well as stamped on a fresh install — every statement of one is a `DROP … IF EXISTS` or a `DELETE FROM extension_migrations`, so a truly fresh database runs it as a no-op while a database meeting the extension for the first time actually drops what a deleted extension left behind. `is_retirement` is exported.

### Removed

- The two redundant indexes on `extension_migrations` (migration `001_prune_prefix_duplicate_indexes`): `(extension_id)` inside `(extension_id, version)` inside the `UNIQUE` constraint on the same columns. Every extension install writes this table, so both extras were write cost with no read they served alone.

## [0.58.0] - 2026-09-21

### Added

- Routine pre-pass: every extension's declarative `CREATE OR REPLACE FUNCTION` is applied (with `check_function_bodies = off`) after the structural phase and before any migration runs, so a migration may reference a function only a declarative schema defines. The dependent phase re-issues each routine with bodies checked against the migrated schema. A declarative `CREATE FUNCTION` without `OR REPLACE` is refused at prepare time. A function that already exists with another signature (`42P13`) is left untouched by the pre-pass — the migration that reshapes it runs next, and the dependent phase fails if none did.
- The cross-extension ALTER check counts a table an earlier migration of the same extension created as the extension's own, so a chain that creates, alters and later drops a table replays on a database older than all three (web 074/077/085 refused at 077 on a 0.52 database).
- Verified checksum transition honours `Migration::supersedes`: a tracking row holding the superseded checksum is moved to the current one without running SQL, and `verify_checksum` accepts either.
- `check_migration_references`: before any database write, a migration that names a trigger or view only a declarative schema file creates — `ALTER TABLE … ENABLE/DISABLE TRIGGER`, `DROP TRIGGER`/`DROP VIEW` without `IF EXISTS`, or a query over the view — fails with `LoaderError::MigrationReferencesDeclarativeObject`. `DO $$ … $$` bodies are not scanned, which makes a catalog-guarded reference inside one the sanctioned form. Replaces the `lint-migration-refs` shell gate. Two shipped migrations (mcp 010, marketplace 012) had this shape and broke self-host upgrades from a database that had never booted on the schema defining the object.

## [0.56.0] - 2026-09-18

### Added

- `ServiceRepository::update_service_port(service_name, port: u16)`: rewrites `services.port` (and `updated_at`) for this instance's row; used by the API proxy resolver to reconcile a stale port.

## [0.55.0] - 2026-09-17

### Added

- `admin::introspection` counts rows exactly (`COUNT(*)`) on request beside the planner estimate.
- `services::postgres::conversion` renders `regclass`, `"char"` and `oid` values (previously blank).

## [0.53.0] - 2026-09-15

### Breaking

- **Breaking:** `DatabaseProvider::get_postgres_pool` returns `Arc<PgPool>` and `is_postgres` is removed; only Postgres exists. `Database::pool`/`write_pool` are infallible. Migrate by dropping the `Option` handling.
- **Breaking:** `Database::from_config`/`from_config_with_write` are replaced by `Database::connect(read_url, write_url, &pool_config)`; the `db_type` string is gone. Migrate by removing the first argument.
- **Breaking:** `CircuitBreaker::acquire` returns an RAII `Probe` that must be settled with `success()`/`failure()`; `ResilienceGuard::acquire_permit` is replaced by `admit()`, which returns `Admission { permit, probe }`. Migrate by settling the probe instead of calling `record_success`/`record_failure` on the breaker.
- **Breaking:** `install_extension_schemas*` return a `SchemaInstallReport` whose `foreign_key_drift` lists every declared foreign key an established database could not create, for `/health/detail` and `infra db migrate` to surface.
- **Breaking:** `AdminSqlError::ForbiddenKeyword` is replaced by `WriteInReadOnly` and a `Parse(pg_query::Error)` variant; `QueryExecutorError::WriteQueryNotAllowed` (never constructed) is removed.
- **Breaking:** `lint_declarative_schema(s)` return `Result<Vec<LintError>, Vec<LintError>>` — `Ok` carries the warnings that were previously discarded; `created_table_names` returns `Result<Vec<String>, pg_query::Error>` with schema-qualified names, instead of an empty list on a parse failure.
- **Breaking:** `split_create_table_foreign_keys` returns `FkDeferralError` instead of `String`.
- **Breaking:** `CleanupRepository` is removed. Log retention lives on `systemprompt_logging::LoggingRepository` (`delete_orphaned_logs`, `count_orphaned_logs`); OAuth expiry sweeps live on `systemprompt_oauth::repository::OauthCleanupRepository`.
- **Breaking:** `SqlExecutor::table_exists`/`column_exists` are removed; use `validate_table_exists`/`validate_column_exists`. `with_transaction`/`with_scoped_transaction` take `&PgPool` and the `_raw` variants are removed.
- **Breaking:** `DatabaseProvider::fetch_scalar_value` is removed (no caller; it conflated NULL with an unrepresentable number).
- **Breaking:** `RepositoryError` gains `SqlSplit`, `Statement { statement, source }`, `Connection` and `SqlFile { path, source }` variants; `SqlExecutor` and `validate_database_connection` no longer flatten causes into `Internal(String)`.

### Added

- `BootstrapLockGuard` and `BOOTSTRAP_ADVISORY_LOCK_KEY` are public; `PostgresProvider` exposes `connection::connect_options`.
- The database extension now declares the `services` process-registry table (moved from the agent extension, whose migrations declare it as a cross-extension table).
- `RepositoryError::is_serialization_failure` — the retry classifier `with_transaction_retry` uses, exposed for callers that run their own retry loop.

### Changed

- A failed write to the CLI display sink is reported through `tracing::warn!`.
- `SqlExecutor::parse_sql_statements` splits with the Postgres parser (`pg_query::split_with_parser`): quoted identifiers and escape strings containing `;` no longer split, and malformed SQL is refused as a whole.
- `SqlExecutor::execute_file` reads asynchronously.
- The schema linter compares identifiers exactly (pg_query already case-folds unquoted names), keys tables by schema, and compares unique-key column sets as deduplicated sets; the referenced-uniqueness message states the project rule.
- Column introspection is scoped to `table_schema = 'public'`.
- Serving-pool connections disable sqlx's prepared-statement cache (`statement_cache_capacity(0)`) so DDL applied by migrations on the same pool cannot leave a connection with a stale cached plan (SQLSTATE 0A000); every query is prepared per execution.
- `DatabaseHandle::is_connected` reports whether both pools are open instead of a constant `true`.
- Migration checksums are xxh64: an applied row whose stored checksum is the historical `DefaultHasher` digest of the SQL now in its slot is rewritten to the xxh64 digest in one verified transaction (`checksum_transition`) — never executed, never a drift repair — and any other mismatch is still checksum drift.
- Repeated schema installation is idempotent; the schema linter skips dollar-quoted function bodies.

### Fixed

- A database URL carrying `sslmode=verify-full`/`verify-ca` (and `sslrootcert`) is honoured; the provider no longer downgrades every mode other than `require`/`disable` to `prefer`.
- On an established database only the `ADD CONSTRAINT` of a declared foreign key may be recorded as drift; a failing catalog probe, savepoint or release now fails the install instead of aborting the transaction silently and reporting success.
- A `BootstrapLockGuard` dropped without `release` (cancelled or panicking install) closes its session so the advisory lock cannot survive in the pool for up to `max_lifetime`; an explicit `release` whose `pg_advisory_unlock` fails closes the session too instead of returning the lock-holding connection to the pool.
- A deferred foreign key whose `VALIDATE CONSTRAINT` fails is reported with the failure cause; the warning no longer asserts that existing rows violate the key when the validation timed out or lost its lock.
- A circuit-breaker probe whose future is cancelled releases its half-open slot; previously the leaked probes left the breaker open forever.
- `AdminSql::parse_readonly` parses with `pg_query` and refuses a data-modifying CTE (`WITH d AS (DELETE …) SELECT …`), DDL or utility statement anywhere in the tree; the keyword heuristic let them through. Read-only admin queries also run inside a `READ ONLY` transaction.
- `infra db query` decodes `uuid`, `numeric` and `bytea` columns instead of returning `null` for them.
- A `DEFERRABLE`/`INITIALLY …` attribute after a `UNIQUE`/`PRIMARY KEY` that follows the column's `REFERENCES` stays on that constraint, as Postgres attaches it, instead of being folded into the deferred foreign key.
- A malformed `extension_migrations` row fails the migration run instead of being skipped and re-executed as pending.

## [0.52.0] - 2026-09-14

### Removed

- `PostgresProvider` no longer reads `PGCA_CERT_PATH` to install a TLS root certificate. The variable was undocumented and set by nothing; a private CA belongs in the database URL (`?sslrootcert=/path/ca.pem`), which sqlx honours.

## [0.48.0] - 2026-09-08

### Changed

- No functional change; the crate's `why` comments were re-cut by the comment-standards pass to state the hidden constraint and nothing else.

## [0.47.0] - 2026-09-06

### Added

- Migration tombstones reserve retired numbers or ranges at build time. The runner neither executes nor records tombstones; fresh installs skip them and existing migration records remain intact.
- Applied migration names are checked against their slot. Collisions return `MigrationSlotReused`, with both names, and follow `--allow-checksum-drift` handling. Unclaimed applied versions produce nonfatal orphan warnings; matching tombstones clear them.
- `refuse_slot_collisions` is public, so the CLI's dry-run refusal can reach the same decision the runner makes rather than reimplementing it.

### Changed

- **Operator-visible:** migration repair rejects slot collisions and directs operators to tombstones. Status distinguishes collisions from checksum drift and labels affected rows.
- `RepairResult` carries `reapplied` separately from `migrations_run`. `--apply` printed "0 migration(s) re-applied" because the count only ever covered newly-applied pending migrations, never the drifted ones it had just re-executed. The reconcile-only path prints a warning stating plainly that no SQL was executed, in place of an unqualified success.
- The two slot-verification methods moved to a sibling `verify.rs`, following the layout this directory already uses for `down`/`exec`/`repair`/`stamp`/`status`. Both compare a stored row against the file now in its slot and both are downgraded by `--allow-checksum-drift`; the distinction between them is worth a module head. No behaviour change.

### Fixed

- `baseline_stamp_rows` supplies fresh-install records that commit in the same transaction as each extension’s structural DDL.
- Tombstone ranges bypass migration-name identity checks; existing records retain the names of the migrations originally applied.
- `PendingMigration::no_tx` was hardcoded `false` at both construction sites, so every no-transaction migration was reported as transactional.

## [0.44.0] - 2026-09-02

### Added

- A write pool distinct from the read pool, so a lookup whose staleness would change an authorization outcome can ask for the primary explicitly. `just lint-authoritative-reads` gates which repositories must.
- `services.heartbeat_at` and the queries behind the 15 s per-replica heartbeat and the `service_registry_gc` reaper.

### Changed

- The MCP service registry is keyed `(instance_id, name)`, so a replica registers, judges staleness and reaps only its own rows.

## [0.42.0] - 2026-08-31

### Changed

- Identifier quoting moved onto the validated `Identifier` type as `quoted()`, so it cannot be reached with an unvalidated string. It had been a free function duplicated across the admin and `services/postgres` introspection modules, and the second copy took a bare `&str` — correct only because of where its caller happened to obtain the value, which is a property that survives exactly until the next caller.

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
