//! Trait abstractions over a database backend.
//!
//! [`DatabaseProvider`] is the dyn-safe surface (callers hold
//! `Arc<dyn DatabaseProvider>`), so it uses `#[async_trait]` for object
//! safety. [`DatabaseProviderExt`] is generic and never used through a
//! trait object; it uses native `async fn` and is intentionally not
//! `#[async_trait]`.

use crate::models::{
    DatabaseInfo, DatabaseTransaction, DbValue, FromDatabaseRow, JsonRow, QueryResult,
    QuerySelector, ToDbValue,
};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

/// Dyn-safe trait abstracting query and transaction primitives over a
/// database backend.
///
/// All call sites that need to be type-erased (extension code, the `AppContext`
/// store, the async migration helpers) hold an `Arc<dyn DatabaseProvider>`.
/// Returning `anyhow::Result` keeps the trait usable by upstream extension
/// code that does not depend on `systemprompt-database`'s typed
/// [`crate::RepositoryError`]; the typed error is only used by non-trait
/// public APIs.
#[async_trait]
pub trait DatabaseProvider: Send + Sync + std::fmt::Debug {
    /// Borrow the underlying `PostgreSQL` pool, if this provider exposes one.
    fn get_postgres_pool(&self) -> Option<Arc<sqlx::PgPool>> {
        None
    }

    /// Always true for the current provider matrix; reserved for future
    /// non-`PostgreSQL` providers.
    fn is_postgres(&self) -> bool {
        true
    }

    /// Execute a non-returning statement and return the affected row count.
    async fn execute(&self, query: &dyn QuerySelector, params: &[&dyn ToDbValue]) -> Result<u64>;

    /// Execute a single raw SQL statement with no parameters.
    async fn execute_raw(&self, sql: &str) -> Result<()>;

    /// Fetch all matching rows.
    async fn fetch_all(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> Result<Vec<JsonRow>>;

    /// Fetch a single row (errors if zero or more than one match).
    async fn fetch_one(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> Result<JsonRow>;

    /// Fetch zero-or-one row.
    async fn fetch_optional(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> Result<Option<JsonRow>>;

    /// Fetch the first column of the first row as a [`DbValue`].
    async fn fetch_scalar_value(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> Result<DbValue>;

    /// Begin a transaction.
    async fn begin_transaction(&self) -> Result<Box<dyn DatabaseTransaction>>;

    /// Return server version, table list, and database size.
    async fn get_database_info(&self) -> Result<DatabaseInfo>;

    /// Round-trip a `SELECT 1` to verify the connection is live.
    async fn test_connection(&self) -> Result<()>;

    /// Execute every statement in `sql` in sequence (batch DDL).
    async fn execute_batch(&self, sql: &str) -> Result<()>;

    /// Run `query` with no parameters and return a dynamic [`QueryResult`].
    async fn query_raw(&self, query: &dyn QuerySelector) -> Result<QueryResult>;

    /// Run `query` with JSON-typed dynamic parameters and return a dynamic
    /// [`QueryResult`]. Parameters are intentionally `serde_json::Value` so
    /// the same call site can carry heterogeneous admin parameter sets.
    async fn query_raw_with(
        &self,
        query: &dyn QuerySelector,
        params: Vec<serde_json::Value>,
    ) -> Result<QueryResult>;
}

/// Generic, non-`dyn` extension over [`DatabaseProvider`] that decodes rows
/// into `T: FromDatabaseRow`. Callers hold the concrete provider type, so
/// native `async fn` is used.
///
/// The `async_fn_in_trait` lint is silenced because this trait is never used
/// through a `dyn` object — `Send` bounds on the returned future are
/// inferred from the concrete implementor, which is exactly what we want.
#[allow(async_fn_in_trait)]
pub trait DatabaseProviderExt {
    /// Fetch zero-or-one row decoded into `T`.
    async fn fetch_typed_optional<T: FromDatabaseRow>(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> Result<Option<T>>;

    /// Fetch a single row decoded into `T`.
    async fn fetch_typed_one<T: FromDatabaseRow>(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> Result<T>;

    /// Fetch all matching rows decoded into `T`.
    async fn fetch_typed_all<T: FromDatabaseRow>(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> Result<Vec<T>>;
}
