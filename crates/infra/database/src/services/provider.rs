//! Trait abstractions over a database backend.
//!
//! [`DatabaseProvider`] is dyn-safe (callers hold `Arc<dyn DatabaseProvider>`)
//! and uses `#[async_trait]`. [`DatabaseProviderExt`] is generic, never used
//! through a trait object, and uses native `async fn`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::DatabaseResult;
use crate::models::{
    DatabaseInfo, DatabaseTransaction, DbValue, FromDatabaseRow, JsonRow, QueryResult,
    QuerySelector, ToDbValue,
};
use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait DatabaseProvider: Send + Sync + std::fmt::Debug {
    fn get_postgres_pool(&self) -> Option<Arc<sqlx::PgPool>> {
        None
    }

    fn is_postgres(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<u64>;

    async fn execute_raw(&self, sql: &str) -> DatabaseResult<()>;

    async fn fetch_all(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<Vec<JsonRow>>;

    async fn fetch_one(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<JsonRow>;

    async fn fetch_optional(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<Option<JsonRow>>;

    async fn fetch_scalar_value(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<DbValue>;

    async fn begin_transaction(&self) -> DatabaseResult<Box<dyn DatabaseTransaction>>;

    async fn get_database_info(&self) -> DatabaseResult<DatabaseInfo>;

    async fn test_connection(&self) -> DatabaseResult<()>;

    async fn execute_batch(&self, sql: &str) -> DatabaseResult<()>;

    async fn query_raw(&self, query: &dyn QuerySelector) -> DatabaseResult<QueryResult>;

    async fn query_raw_with(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<QueryResult>;
}

#[expect(
    async_fn_in_trait,
    reason = "internal extension trait used statically; no dyn dispatch, so no Send-bound concern"
)]
pub trait DatabaseProviderExt {
    async fn fetch_typed_optional<T: FromDatabaseRow>(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<Option<T>>;

    async fn fetch_typed_one<T: FromDatabaseRow>(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<T>;

    async fn fetch_typed_all<T: FromDatabaseRow>(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<Vec<T>>;
}
