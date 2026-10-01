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
//! - [`DatabaseResult`] — result alias over the workspace's single
//!   [`systemprompt_traits::RepositoryError`].
//! - [`MigrationService`], [`install_extension_schemas_full`] — lifecycle
//!   helpers driving extension-supplied DDL.
//! - [`DatabaseAdminService`], [`QueryExecutor`], [`AdminSql`],
//!   [`SafeIdentifier`] — admin/introspection layer used by the CLI.
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
//! two paths whose contract is dynamic SQL by design; the `lint-sqlx` gate
//! allows runtime `sqlx::query` only there:
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

pub use extension::DatabaseExtension;

pub use models::{
    ColumnInfo, DatabaseInfo, DatabaseQuery, DatabaseTransaction, DbValue, FromDbValue, IndexInfo,
    JsonRow, QueryResult, QueryRow, QuerySelector, TableInfo, ToDbValue, parse_database_datetime,
};

pub use services::{
    BoxFuture, Database, DatabaseCliDisplay, DatabaseExt, DatabaseProvider, DbPool, PoolConfig,
    PostgresProvider, SqlExecutor, with_transaction_retry,
};

pub use error::DatabaseResult;
pub use lifecycle::{
    AppliedMigration, BOOTSTRAP_ADVISORY_LOCK_KEY, BaselineStamp, BootstrapLockGuard,
    ChecksumDrift, DeferredForeignKey, ExpensiveStatement, ExtensionMigrationStatus,
    FkDeferralError, ForeignKeyDrift, FreshnessCheck, HOT_TABLES, MarkAppliedOutcome,
    MigrationConfig, MigrationCost, MigrationResult, MigrationService, MigrationStatus,
    OrphanMigrationLedger, OrphanedMigration, PendingMigration, RepairResult, ReplicaStatus,
    SchemaInstallReport, SchemaResidue, SlotCollision, SplitCreateTable, TombstonedSlot,
    UndeclaredTable, audit_migration_cost, audit_one, audit_schema_residue,
    check_migration_references, check_trigger_routines, install_extension_schemas_full,
    is_retirement, replica_status, split_create_table_foreign_keys, validate_column_exists,
    validate_database_connection, validate_table_exists, validate_write_pool_is_primary,
};
pub use repository::{
    CreateServiceInput, PgDbPool, ServiceConfig, ServiceModule, ServiceRepository, ServiceStatus,
    UpsertServiceProcessInput,
};

pub use admin::{
    AdminSql, AdminSqlError, DEFAULT_READONLY_ROW_LIMIT, DatabaseAdminService, IdentifierError,
    QueryExecutor, QueryExecutorError, SafeIdentifier,
};

use systemprompt_traits::DatabaseHandle;

impl DatabaseHandle for Database {
    fn is_connected(&self) -> bool {
        !self.pool().is_closed() && !self.write_pool().is_closed()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
