# systemprompt-core-database

PostgreSQL database abstraction layer.

## Structure

```
database/
├── Cargo.toml
├── module.yml
├── schema/
│   └── functions.sql          # Shared PostgreSQL functions
└── src/
    ├── lib.rs                  # Crate root, public exports
    ├── error.rs                # RepositoryError type
    ├── models/
    │   ├── mod.rs              # Module exports
    │   ├── info.rs             # DatabaseInfo, TableInfo, ColumnInfo
    │   ├── query.rs            # DatabaseQuery, QuerySelector, FromDatabaseRow, QueryResult
    │   └── transaction.rs      # DatabaseTransaction trait
    ├── repository/
    │   ├── mod.rs              # Module exports
    │   ├── base.rs             # Repository trait, PgDbPool, PaginatedRepository
    │   ├── info.rs             # DatabaseInfoRepository
    │   └── macros.rs           # impl_repository_new!, define_repository!, impl_repository_pool!
    └── services/
        ├── mod.rs              # Module exports
        ├── database.rs         # Database wrapper, DbPool, DatabaseExt
        ├── display.rs          # DatabaseCliDisplay trait
        ├── executor.rs         # SqlExecutor utility
        ├── provider.rs         # DatabaseProvider, DatabaseProviderExt traits
        ├── transaction.rs      # with_transaction, with_transaction_retry helpers
        └── postgres/
            ├── mod.rs          # PostgresProvider implementation
            ├── conversion.rs   # row_to_json, bind_params, rows_to_result
            ├── ext.rs          # DatabaseProviderExt implementation
            ├── introspection.rs # get_database_info
            └── transaction.rs  # PostgresTransaction implementation
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
