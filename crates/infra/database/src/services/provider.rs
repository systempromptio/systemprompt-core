use crate::models::{
    DatabaseInfo, DatabaseTransaction, DbValue, FromDatabaseRow, JsonRow, QueryResult,
    QuerySelector, ToDbValue,
};
use anyhow::Result;
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

    async fn execute(&self, query: &dyn QuerySelector, params: &[&dyn ToDbValue]) -> Result<u64>;

    async fn execute_raw(&self, sql: &str) -> Result<()>;

    async fn fetch_all(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> Result<Vec<JsonRow>>;

    async fn fetch_one(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> Result<JsonRow>;

    async fn fetch_optional(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> Result<Option<JsonRow>>;

    async fn fetch_scalar_value(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> Result<DbValue>;

    async fn begin_transaction(&self) -> Result<Box<dyn DatabaseTransaction>>;

    async fn get_database_info(&self) -> Result<DatabaseInfo>;

    async fn test_connection(&self) -> Result<()>;

    async fn execute_batch(&self, sql: &str) -> Result<()>;

    async fn query_raw(&self, query: &dyn QuerySelector) -> Result<QueryResult>;

    async fn query_raw_with(
        &self,
        query: &dyn QuerySelector,
        params: Vec<serde_json::Value>,
    ) -> Result<QueryResult>;
}

#[allow(async_fn_in_trait)]
pub trait DatabaseProviderExt {
    async fn fetch_typed_optional<T: FromDatabaseRow>(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> Result<Option<T>>;

    async fn fetch_typed_one<T: FromDatabaseRow>(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> Result<T>;

    async fn fetch_typed_all<T: FromDatabaseRow>(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> Result<Vec<T>>;
}
