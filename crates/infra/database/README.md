<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo.svg" alt="systemprompt.io" width="180">
</picture>

### Production infrastructure for AI agents

[**Website**](https://systemprompt.io) · [**Documentation**](https://systemprompt.io/documentation/) · [**Guides**](https://systemprompt.io/guides) · [**Core**](https://github.com/systempromptio/systemprompt-core) · [**Template**](https://github.com/systempromptio/systemprompt-template) · [**Discord**](https://discord.gg/wkAbSuPWpr)

</div>

---

# systemprompt-database

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/00-overview.svg">
    <img alt="systemprompt-database — systemprompt-core workspace" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-database.svg?style=flat-square)](https://crates.io/crates/systemprompt-database)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-database?style=flat-square)](https://docs.rs/systemprompt-database)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

One PostgreSQL holds every prompt, key, and audit record your deployment produces, on infrastructure you run. This crate is how the rest of the workspace reaches it: a pooled `SQLx` handle, generic repository traits, and compile-time-verified queries.

**Layer**: Infra. Infrastructure primitives consumed by the domain and application crates. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## What it does

Every service reaches the database through the same audited path. Static SQL goes through the compile-time-verified `sqlx::query!` / `query_as!` / `query_scalar!` macros; dynamic SQL is contained to two allowlisted surfaces (the admin CLI and the provider implementation) where dynamic SQL is the contract. Domain crates never write raw queries; they build on the repository traits.

The crate owns the pool, the transaction helpers, the extension migration runner, and the resilience primitives (circuit breaker, bulkhead, retry) that wrap its own connection and transaction attempts.

## Modules

| Module | Purpose |
|--------|---------|
| `services` | The pool (`Database`, `DbPool`), the dyn-safe `DatabaseProvider` and its `PostgresProvider` implementation, the `SqlExecutor`, transaction helpers, CLI display, and the `schema_linter/` directory that rejects imperative DDL in `schema/*.sql` at boot. |
| `repository` | Repository pattern building blocks: the `Repository` / `PaginatedRepository` traits and macros, `CleanupRepository`, and the `service/` process-registration repository. |
| `lifecycle` | Schema installation (`installation/`), the migration runner (`migrations/` — apply, mark-applied, repair, squash, status), and connection/table/column validation. |
| `models` | Query, transaction, and introspection data types (`DatabaseQuery`, `DatabaseTransaction`, `DatabaseInfo`, `TableInfo`, `ColumnInfo`, `IndexInfo`). |
| `admin` | Constrained admin surfaces: `DatabaseAdminService` introspection, the read-only `QueryExecutor`, `AdminSql` builders, and `SafeIdentifier` validation. |
| `resilience` | Domain-agnostic resilience primitives (`ResilienceGuard`, `CircuitBreaker`, `Bulkhead`, `retry_async`, classify, stream) that wrap outbound calls; the crate's own retries run on them. |
| `squash_baseline` | `SquashBaselineService` locates an extension's source crate and writes squashed migration baselines; filesystem-only, with its own `SquashBaselineError`. |
| `extension` | `DatabaseExtension` registers the crate's base schema (`functions.sql`, `extension_migrations.sql`) through the workspace extension framework. |
| `error` | `RepositoryError` and `DatabaseResult<T>`. |

Schema DDL lives in `schema/` (`functions.sql`, `extension_migrations.sql`).

## Usage

```toml
[dependencies]
systemprompt-database = "0.21"
```

```rust
use systemprompt_database::{DatabaseResult, DbPool, with_transaction};

async fn example(pool: &DbPool) -> DatabaseResult<()> {
    with_transaction(pool, |tx| Box::pin(async move {
        // Execute queries within transaction
        Ok(())
    })).await
}
```

```rust
use systemprompt_database::{DatabaseResult, DbPool, with_transaction};

async fn count_users(pool: &DbPool) -> DatabaseResult<i64> {
    with_transaction(pool, |tx| Box::pin(async move {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(&mut **tx)
            .await?;
        Ok(row.0)
    }))
    .await
}
```

## Public API

### Types

| Type | Source | Description |
|------|--------|-------------|
| `Database` | `services/database.rs` | Main database wrapper |
| `DbPool` | `services/database.rs` | `Arc<Database>` alias |
| `PgDbPool` | `repository/base.rs` | `Arc<PgPool>` alias for repositories |
| `RepositoryError` | `error.rs` | Unified repository error type |
| `PostgresProvider` | `services/postgres/mod.rs` | PostgreSQL provider |
| `PostgresTransaction` | `services/postgres/transaction.rs` | Transaction handle |
| `SqlExecutor` | `services/executor.rs` | Statement splitter and executor |
| `DatabaseQuery` | `models/query.rs` | Static query wrapper |
| `QueryResult` | `models/query.rs` | Query execution result |
| `QueryRow` | `models/query.rs` | Row representation |
| `DatabaseInfo` | `models/info.rs` | Database metadata |
| `TableInfo` | `models/info.rs` | Table metadata |
| `ColumnInfo` | `models/info.rs` | Column metadata |
| `IndexInfo` | `models/info.rs` | Index metadata |
| `DatabaseAdminService` | `admin/introspection.rs` | Admin introspection service |
| `QueryExecutor` | `admin/query_executor.rs` | Constrained admin query executor |
| `AdminSql` | `admin/admin_sql.rs` | Vetted admin SQL builders |
| `SafeIdentifier` | `admin/identifier.rs` | Validated SQL identifier wrapper |
| `MigrationService` | `lifecycle/migrations/` | Extension migration runner |
| `MigrationConfig` | `lifecycle/migrations/` | Migration runner configuration |
| `MigrationStatus` | `lifecycle/migrations/` | Per-migration state |
| `MigrationResult` | `lifecycle/migrations/` | Migration run outcome |
| `AppliedMigration` | `lifecycle/migrations/` | Applied migration record |
| `SquashBaselineService` | `squash_baseline.rs` | Locates an extension crate and writes squashed migration baselines |
| `SquashBaselineError` | `squash_baseline.rs` | Filesystem error type for baseline squashing |
| `DatabaseExtension` | `extension.rs` | Extension trait implementation for this crate |
| `BoxFuture` | `services/transaction.rs` | Boxed future type for transactions |

### Traits

| Trait | Source | Description |
|-------|--------|-------------|
| `Repository` | `repository/base.rs` | Base CRUD repository trait |
| `PaginatedRepository` | `repository/base.rs` | Pagination extension trait |
| `DatabaseProvider` | `services/provider.rs` | Core database operations |
| `DatabaseProviderExt` | `services/provider.rs` | Typed row fetching |
| `DatabaseTransaction` | `models/transaction.rs` | Transaction operations |
| `QuerySelector` | `models/query.rs` | Query abstraction |
| `FromDatabaseRow` | `models/query.rs` | Row-to-type conversion |
| `DatabaseExt` | `services/database.rs` | Database extraction |
| `DatabaseCliDisplay` | `services/display.rs` | CLI output formatting |

### Functions

| Function | Source | Description |
|----------|--------|-------------|
| `with_transaction` | `services/transaction.rs` | Execute closure in transaction |
| `with_transaction_raw` | `services/transaction.rs` | Transaction with raw `PgPool` |
| `with_transaction_retry` | `services/transaction.rs` | Transaction with automatic retry |
| `install_extension_schemas` | `lifecycle/installation/mod.rs` | Install all registered extension schemas |
| `install_extension_schemas_full` | `lifecycle/installation/mod.rs` | Install schemas and run pending migrations |
| `install_extension_schemas_with_config` | `lifecycle/installation/mod.rs` | Install schemas honouring a `MigrationConfig` |
| `validate_database_connection` | `lifecycle/validation.rs` | Probe the live connection |
| `validate_table_exists` | `lifecycle/validation.rs` | Assert a table is present |
| `validate_column_exists` | `lifecycle/validation.rs` | Assert a column is present |
| `parse_database_datetime` | re-export from `systemprompt-traits` | Parse driver-native datetime values |

### Macros

| Macro | Source | Description |
|-------|--------|-------------|
| `impl_repository_new!` | `repository/base.rs` | Generate `new()` constructor for repositories |
| `define_repository!` | `repository/base.rs` | Define repository struct with pool field |
| `impl_repository_pool!` | `repository/base.rs` | Generate pool accessor methods |

### Resilience

The `resilience` module is publicly exported and domain-agnostic. `ResilienceGuard`, `CircuitBreaker`, `Bulkhead`, and `retry_async` wrap outbound calls; the crate's own connection and transaction retries run on them.

### Re-exports

From `systemprompt-traits`: `DbValue`, `ToDbValue`, `FromDbValue`, `JsonRow`, `parse_database_datetime`

From `systemprompt-identifiers`: `UserId`, `TaskId`, `SessionId`, `ContextId`, `TraceId`, `ArtifactId`, `ExecutionStepId`, `SkillId`, `ContentId`, `FileId`, `ClientId`, `TokenId`, `LogId`

From `sqlx`: `PgPool`, `Pool`, `Postgres`, `Transaction`, `Json`

## Dependencies

- `systemprompt-traits` — core traits
- `systemprompt-identifiers` — typed identifiers
- `systemprompt-models` — shared model types
- `systemprompt-extension` — extension framework registration
- `sqlx` — PostgreSQL driver with compile-time-verified macros
- `tokio` — async runtime
- `serde` / `serde_json` — serialization
- `chrono` — timestamps
- `uuid` — UUID support
- `rust_decimal` — decimal support
- `base64` — encoded value support
- `thiserror` — error derivation
- `async-trait` — `dyn`-compatible async traits
- `tracing` — structured logging
- `inventory` — extension registration

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-database)** · **[docs.rs](https://docs.rs/systemprompt-database)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Infra layer · Own how your organization uses AI.</sub>

</div>
