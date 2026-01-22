# systemprompt-database

Database abstraction layer for systemprompt.io supporting SQLite, PostgreSQL, and MySQL.

[![Crates.io](https://img.shields.io/crates/v/systemprompt-database.svg)](https://crates.io/crates/systemprompt-database)
[![Documentation](https://docs.rs/systemprompt-database/badge.svg)](https://docs.rs/systemprompt-database)
[![License: FSL-1.1-ALv2](https://img.shields.io/badge/License-FSL--1.1--ALv2-blue.svg)](LICENSE)

## Overview

**Part of the Infra layer in the systemprompt.io architecture.**

Provides database abstraction via SQLx with repository patterns, transaction helpers, and administrative utilities.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
systemprompt-database = "0.0.1"
```

## Quick Example

```rust
use systemprompt_database::{DbPool, Repository, with_transaction};

async fn example(pool: &DbPool) -> anyhow::Result<()> {
    with_transaction(pool, |tx| Box::pin(async move {
        // Execute queries within transaction
        Ok(())
    })).await
}
```

## Structure

```
database/
├── Cargo.toml
├── module.yml
├── README.md
├── status.md
├── schema/
│   └── functions.sql
└── src/
    ├── lib.rs
    ├── error.rs
    ├── admin/
    │   ├── mod.rs
    │   ├── introspection.rs
    │   └── query_executor.rs
    ├── lifecycle/
    │   ├── mod.rs
    │   ├── installation.rs
    │   └── validation.rs
    ├── models/
    │   ├── mod.rs
    │   ├── info.rs
    │   ├── query.rs
    │   └── transaction.rs
    ├── repository/
    │   ├── mod.rs
    │   ├── base.rs
    │   ├── cleanup.rs
    │   ├── entity.rs
    │   ├── info.rs
    │   ├── macros.rs
    │   └── service.rs
    └── services/
        ├── mod.rs
        ├── database.rs
        ├── display.rs
        ├── executor.rs
        ├── provider.rs
        ├── transaction.rs
        └── postgres/
            ├── mod.rs
            ├── conversion.rs
            ├── ext.rs
            ├── introspection.rs
            └── transaction.rs
```

## Modules

### `admin/`
Administrative database utilities for introspection and query execution.

| File | Purpose |
|------|---------|
| `introspection.rs` | `DatabaseAdminService` for listing tables, describing columns, getting indexes |
| `query_executor.rs` | `QueryExecutor` for safe SQL execution with read-only mode support |

### `lifecycle/`
Database setup and validation.

| File | Purpose |
|------|---------|
| `installation.rs` | Schema and seed installation for modules and extensions |
| `validation.rs` | Connection and schema validation functions |

### `models/`
Data structures for database operations.

| File | Purpose |
|------|---------|
| `info.rs` | `DatabaseInfo`, `TableInfo`, `ColumnInfo`, `IndexInfo` |
| `query.rs` | `DatabaseQuery`, `QuerySelector`, `FromDatabaseRow`, `QueryResult` |
| `transaction.rs` | `DatabaseTransaction` trait |

### `repository/`
Repository pattern implementations.

| File | Purpose |
|------|---------|
| `base.rs` | `Repository` trait, `PgDbPool` type alias, `PaginatedRepository` |
| `cleanup.rs` | Cleanup utilities for expired data |
| `entity.rs` | Generic `Entity` trait and `GenericRepository<E>` |
| `info.rs` | `DatabaseInfoRepository` for metadata queries |
| `macros.rs` | `impl_repository_new!`, `define_repository!`, `impl_repository_pool!` |
| `service.rs` | `ServiceRepository` for service process management |

### `services/`
Core database services and providers.

| File | Purpose |
|------|---------|
| `database.rs` | `Database` wrapper, `DbPool`, `DatabaseExt` |
| `display.rs` | `DatabaseCliDisplay` trait for CLI output |
| `executor.rs` | `SqlExecutor` for SQL statement parsing and execution |
| `provider.rs` | `DatabaseProvider`, `DatabaseProviderExt` traits |
| `transaction.rs` | `with_transaction`, `with_transaction_retry` helpers |

### `services/postgres/`
PostgreSQL-specific implementations.

| File | Purpose |
|------|---------|
| `mod.rs` | `PostgresProvider` implementation |
| `conversion.rs` | `row_to_json`, `bind_params`, `rows_to_result` |
| `ext.rs` | `DatabaseProviderExt` implementation |
| `introspection.rs` | `get_database_info` for schema introspection |
| `transaction.rs` | `PostgresTransaction` implementation |

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
| `DatabaseQuery` | `models/query.rs` | Static query wrapper |
| `QueryResult` | `models/query.rs` | Query execution result |
| `DatabaseInfo` | `models/info.rs` | Database metadata |
| `TableInfo` | `models/info.rs` | Table metadata |
| `ColumnInfo` | `models/info.rs` | Column metadata |
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
| `with_transaction_raw` | `services/transaction.rs` | Transaction with raw PgPool |
| `with_transaction_retry` | `services/transaction.rs` | Transaction with automatic retry |

### Macros

| Macro | Source | Description |
|-------|--------|-------------|
| `impl_repository_new!` | `repository/macros.rs` | Generate `new()` constructor for repositories |
| `define_repository!` | `repository/macros.rs` | Define repository struct with pool field |
| `impl_repository_pool!` | `repository/macros.rs` | Generate pool accessor methods |

### Re-exports

From `systemprompt-traits`: `DbValue`, `ToDbValue`, `FromDbValue`, `JsonRow`, `parse_database_datetime`

From `systemprompt-identifiers`: `UserId`, `TaskId`, `SessionId`, `ContextId`, `TraceId`, `ArtifactId`, `ExecutionStepId`, `SkillId`, `ContentId`, `FileId`, `ClientId`, `TokenId`, `LogId`

From `sqlx`: `PgPool`, `Pool`, `Postgres`, `Transaction`, `Json`

## Dependencies

- `systemprompt-traits` - Core traits
- `systemprompt-identifiers` - Typed identifiers
- `sqlx` - PostgreSQL driver
- `tokio` - Async runtime
- `serde` / `serde_json` - Serialization
- `chrono` - Timestamps
- `uuid` - UUID support
- `rust_decimal` - Decimal support
- `anyhow` - Error handling
- `thiserror` - Error derivation
- `async-trait` - Async traits

## License

FSL-1.1-ALv2 - See [LICENSE](../../LICENSE) for details.
