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
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

PostgreSQL infrastructure for systemprompt.io AI governance. SQLx-backed pool, generic repository traits, and compile-time query verification. Provides database abstraction via SQLx with repository patterns, transaction helpers, and administrative utilities.

**Layer**: Infra — infrastructure primitives (database, security, events, etc.) consumed by domain crates. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

Provides database abstraction via SQLx with repository patterns, transaction helpers, and administrative utilities.

## Architecture

```
database/
├── Cargo.toml
├── CHANGELOG.md
├── README.md
├── schema/
│   ├── extension_migrations.sql
│   └── functions.sql
└── src/
    ├── lib.rs
    ├── error.rs
    ├── extension.rs
    ├── admin/
    │   ├── mod.rs
    │   ├── admin_sql.rs
    │   ├── identifier.rs
    │   ├── introspection.rs
    │   └── query_executor.rs
    ├── lifecycle/
    │   ├── mod.rs
    │   ├── migrations.rs
    │   ├── validation.rs
    │   └── installation/
    │       ├── mod.rs
    │       └── extension.rs
    ├── models/
    │   ├── mod.rs
    │   ├── info.rs
    │   ├── query.rs
    │   └── transaction.rs
    ├── repository/
    │   ├── mod.rs
    │   ├── base.rs
    │   ├── cleanup.rs
    │   ├── info.rs
    │   └── service/
    │       ├── mod.rs
    │       ├── model.rs
    │       └── repo.rs
    └── services/
        ├── mod.rs
        ├── database.rs
        ├── display.rs
        ├── executor.rs
        ├── provider.rs
        ├── schema_linter.rs
        ├── transaction.rs
        └── postgres/
            ├── mod.rs
            ├── conversion.rs
            ├── ext.rs
            ├── introspection.rs
            └── transaction.rs
```

### `extension.rs`
`DatabaseExtension` implementation that registers the crate's base schema (`functions.sql`, `extension_migrations.sql`) via the workspace extension framework.

### `admin/`
Administrative database utilities for introspection and constrained query execution.

| File | Purpose |
|------|---------|
| `admin_sql.rs` | `AdminSql` builders for vetted dynamic admin queries |
| `identifier.rs` | `SafeIdentifier` validation for user-supplied SQL identifiers |
| `introspection.rs` | `DatabaseAdminService` for listing tables, describing columns, and reading indexes |
| `query_executor.rs` | `QueryExecutor` for SQL execution with read-only mode support |

### `lifecycle/`
Database setup, migration, and validation.

| File | Purpose |
|------|---------|
| `installation/mod.rs` | `install_extension_schemas`, `install_extension_schemas_full`, `install_extension_schemas_with_config` entry points |
| `installation/extension.rs` | Per-extension schema installation pipeline |
| `migrations.rs` | `MigrationService`, `MigrationConfig`, `MigrationStatus`, `MigrationResult`, `AppliedMigration` |
| `validation.rs` | `validate_database_connection`, `validate_table_exists`, `validate_column_exists` |

### `models/`
Data structures for database operations.

| File | Purpose |
|------|---------|
| `info.rs` | `DatabaseInfo`, `TableInfo`, `ColumnInfo`, `IndexInfo` |
| `query.rs` | `DatabaseQuery`, `QuerySelector`, `FromDatabaseRow`, `QueryResult`, `QueryRow` |
| `transaction.rs` | `DatabaseTransaction` trait |

### `repository/`
Repository pattern building blocks.

| File | Purpose |
|------|---------|
| `base.rs` | `Repository` trait, `PaginatedRepository`, `PgDbPool` alias, and repository macros (`impl_repository_new!`, `define_repository!`, `impl_repository_pool!`) |
| `cleanup.rs` | `CleanupRepository` utilities for expired data |
| `info.rs` | `DatabaseInfoRepository` for metadata queries |
| `service/mod.rs` | `ServiceRepository` re-exports |
| `service/model.rs` | `CreateServiceInput`, `ServiceConfig` models |
| `service/repo.rs` | `ServiceRepository` for service process registration |

### `services/`
Core database services and providers.

| File | Purpose |
|------|---------|
| `database.rs` | `Database` wrapper, `DbPool`, `DatabaseExt` |
| `display.rs` | `DatabaseCliDisplay` trait for CLI output |
| `executor.rs` | `SqlExecutor` for hand-rolled byte-state-machine statement splitting and execution |
| `provider.rs` | `DatabaseProvider`, `DatabaseProviderExt` traits |
| `schema_linter.rs` | Boot-time linter that rejects imperative DDL in `schema/*.sql` |
| `transaction.rs` | `with_transaction`, `with_transaction_raw`, `with_transaction_retry`, `BoxFuture` |

### `services/postgres/`
PostgreSQL-specific implementation of the provider surface.

| File | Purpose |
|------|---------|
| `mod.rs` | `PostgresProvider` implementation |
| `conversion.rs` | Row-to-JSON conversion, parameter binding, result mapping |
| `ext.rs` | `DatabaseProviderExt` implementation for `PostgresProvider` |
| `introspection.rs` | `get_database_info` schema introspection |
| `transaction.rs` | `PostgresTransaction` implementation |

## Usage

```toml
[dependencies]
systemprompt-database = "0.13.1"
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
| `MigrationService` | `lifecycle/migrations.rs` | Extension migration runner |
| `MigrationConfig` | `lifecycle/migrations.rs` | Migration runner configuration |
| `MigrationStatus` | `lifecycle/migrations.rs` | Per-migration state |
| `MigrationResult` | `lifecycle/migrations.rs` | Migration run outcome |
| `AppliedMigration` | `lifecycle/migrations.rs` | Applied migration record |
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
