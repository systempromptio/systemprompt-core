//! # systemprompt-database
//!
//! `PostgreSQL` infrastructure for systemprompt.io: a thin `SQLx`-backed pool,
//! generic repository traits, dynamic-query primitives for admin tooling, and
//! lifecycle helpers (schema installation, extension migrations, validation).
//!
//! ## Public API surface
//!
//! - [`Database`] / [`DbPool`] — owned pool wrapper with optional split
//!   read/write providers.
//! - [`DatabaseProvider`] — dyn-safe trait abstracting
//!   query/execute/transaction primitives across providers (currently only
//!   `PostgreSQL`).
//! - [`PostgresProvider`] — the `PostgreSQL` implementation.
//! - [`RepositoryError`] / [`DatabaseResult`] — canonical typed error/result
//!   returned from non-trait public APIs.
//! - [`MigrationService`], [`install_extension_schemas`],
//!   [`install_extension_schemas_full`] — lifecycle helpers driving
//!   extension-supplied DDL.
//! - [`DatabaseAdminService`], [`QueryExecutor`], [`AdminSql`],
//!   [`SafeIdentifier`] — admin/introspection layer used by the CLI.
//! - [`SquashBaselineService`] — locates an extension's source crate in the
//!   workspace layout and writes squashed migration baselines; filesystem-only,
//!   with its own [`SquashBaselineError`].
//! - [`resilience`] — domain-agnostic resilience primitives
//!   ([`resilience::ResilienceGuard`], [`resilience::CircuitBreaker`],
//!   [`resilience::Bulkhead`], [`resilience::retry_async`]) wrapping outbound
//!   calls; the crate's own connection and transaction retries run on them.
//!
//! ## Feature flags
//!
//! This crate currently has no Cargo features; everything compiles
//! unconditionally. The `[package.metadata.docs.rs]` block is in place so
//! `--all-features` documentation builds remain stable as features are added.
//!
//! ## sqlx allowlist
//!
//! Static SQL goes through the compile-time-verified `sqlx::query!` /
//! `query_as!` / `query_scalar!` macros. Runtime/dynamic SQL is contained to
//! two paths whose contract is dynamic SQL by design and that are documented in
//! the workspace allowlist (`ci/check-sqlx.sh`, `instructions/prompt/rust.md`):
//!
//! - `src/admin/` — admin CLI surfaces (introspection, restricted query
//!   executor) where the SQL is the user input.
//! - `src/services/postgres/` — the dyn-safe `DatabaseProvider` implementation,
//!   transaction wrapper, type-erased helpers and `PostgreSQL` schema
//!   introspection.
//!
//! Every other call site uses verified macros.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod admin;
pub mod error;
pub mod extension;
pub mod lifecycle;
pub mod models;
#[macro_use]
pub mod repository;
pub mod resilience;
pub mod services;
pub mod squash_baseline;

pub use extension::DatabaseExtension;

pub use models::{
    ArtifactId, ClientId, ColumnInfo, ContentId, ContextId, DatabaseInfo, DatabaseQuery,
    DatabaseTransaction, DbValue, ExecutionStepId, FileId, FromDatabaseRow, FromDbValue, IndexInfo,
    JsonRow, LogId, QueryResult, QueryRow, QuerySelector, SessionId, SkillId, TableInfo, TaskId,
    ToDbValue, TokenId, TraceId, UserId, parse_database_datetime,
};

pub use services::{
    BoxFuture, Database, DatabaseCliDisplay, DatabaseExt, DatabaseProvider, DatabaseProviderExt,
    DbPool, PoolConfig, PostgresProvider, SqlExecutor, with_transaction, with_transaction_raw,
    with_transaction_retry,
};

pub use error::{DatabaseResult, RepositoryError};
pub use lifecycle::{
    AppliedMigration, ChecksumDrift, ExtensionMigrationStatus, MarkAppliedOutcome, MigrationConfig,
    MigrationResult, MigrationService, MigrationStatus, PendingMigration, RepairResult, SquashPlan,
    install_extension_schemas, install_extension_schemas_full,
    install_extension_schemas_with_config, validate_column_exists, validate_database_connection,
    validate_table_exists,
};
pub use repository::{
    CleanupRepository, CreateServiceInput, PgDbPool, ServiceConfig, ServiceRepository,
};
pub use squash_baseline::{SquashBaselineError, SquashBaselineService};

pub use admin::{
    AdminSql, AdminSqlError, DEFAULT_READONLY_ROW_LIMIT, DatabaseAdminService, IdentifierError,
    QueryExecutor, QueryExecutorError, SafeIdentifier,
};
pub use sqlx::types::Json;
pub use sqlx::{PgPool, Pool, Postgres, Transaction};

use systemprompt_traits::DatabaseHandle;

impl DatabaseHandle for Database {
    fn is_connected(&self) -> bool {
        true
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
