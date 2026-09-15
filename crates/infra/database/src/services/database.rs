//! Top-level [`Database`] handle that owns one or two
//! [`DatabaseProvider`] instances (read + optional write) and exposes the
//! query and transaction surface.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::postgres::PostgresProvider;
use super::postgres::connection::PoolConfig;
use super::provider::DatabaseProvider;
use crate::error::DatabaseResult;
use crate::models::{DatabaseInfo, QueryResult};
use std::sync::Arc;

pub struct Database {
    provider: Arc<dyn DatabaseProvider>,
    write_provider: Option<Arc<dyn DatabaseProvider>>,
}

impl std::fmt::Debug for Database {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Database")
            .field("backend", &"PostgreSQL")
            .finish()
    }
}

impl Database {
    pub async fn new_postgres(url: &str) -> DatabaseResult<Self> {
        let provider = PostgresProvider::new(url).await?;
        Ok(Self {
            provider: Arc::new(provider),
            write_provider: None,
        })
    }

    pub async fn connect(
        read_url: &str,
        write_url: Option<&str>,
        pool: &PoolConfig,
    ) -> DatabaseResult<Self> {
        let provider: Arc<dyn DatabaseProvider> =
            Arc::new(PostgresProvider::new_with_pool(read_url, pool).await?);

        let write_provider: Option<Arc<dyn DatabaseProvider>> = match write_url {
            Some(url) => Some(Arc::new(PostgresProvider::new_with_pool(url, pool).await?)),
            None => None,
        };

        Ok(Self {
            provider,
            write_provider,
        })
    }

    #[must_use]
    pub fn from_pools(read: Arc<sqlx::PgPool>, write: Option<Arc<sqlx::PgPool>>) -> Self {
        let write_provider = write.map(|pool| -> Arc<dyn DatabaseProvider> {
            Arc::new(PostgresProvider::from_pool(pool))
        });
        Self {
            provider: Arc::new(PostgresProvider::from_pool(read)),
            write_provider,
        }
    }

    #[must_use]
    pub fn read(&self) -> &dyn DatabaseProvider {
        self.provider.as_ref()
    }

    #[must_use]
    pub fn write(&self) -> &dyn DatabaseProvider {
        self.write_provider
            .as_deref()
            .unwrap_or_else(|| self.provider.as_ref())
    }

    #[must_use]
    pub fn pool(&self) -> Arc<sqlx::PgPool> {
        self.read().get_postgres_pool()
    }

    #[expect(
        clippy::unnecessary_wraps,
        reason = "every layer threads `?` through this accessor; collapsing its callers onto \
                  `pool()` is a workspace-wide mechanical change scheduled after 0.53.0"
    )]
    pub fn pool_arc(&self) -> DatabaseResult<Arc<sqlx::PgPool>> {
        Ok(self.pool())
    }

    #[must_use]
    pub fn write_pool(&self) -> Arc<sqlx::PgPool> {
        self.write().get_postgres_pool()
    }

    #[expect(
        clippy::unnecessary_wraps,
        reason = "every layer threads `?` through this accessor; collapsing its callers onto \
                  `write_pool()` is a workspace-wide mechanical change scheduled after 0.53.0"
    )]
    pub fn write_pool_arc(&self) -> DatabaseResult<Arc<sqlx::PgPool>> {
        Ok(self.write_pool())
    }

    #[must_use]
    pub fn has_write_pool(&self) -> bool {
        self.write_provider.is_some()
    }

    pub async fn execute_batch(&self, sql: &str) -> DatabaseResult<()> {
        self.write().execute_batch(sql).await
    }

    pub async fn get_info(&self) -> DatabaseResult<DatabaseInfo> {
        self.read().get_database_info().await
    }

    pub async fn test_connection(&self) -> DatabaseResult<()> {
        self.provider.test_connection().await?;
        if let Some(wp) = &self.write_provider {
            wp.test_connection().await?;
        }
        Ok(())
    }

    pub async fn begin(&self) -> DatabaseResult<sqlx::Transaction<'_, sqlx::Postgres>> {
        self.write_pool().begin().await.map_err(Into::into)
    }

    pub async fn begin_scoped(
        &self,
        scope: &systemprompt_models::RequestScope,
    ) -> DatabaseResult<sqlx::Transaction<'static, sqlx::Postgres>> {
        super::scoped_transaction::begin_scoped(&self.write_pool(), scope).await
    }
}

pub type DbPool = Arc<Database>;

pub trait DatabaseExt {
    fn database(&self) -> Arc<Database>;
}

impl DatabaseExt for Arc<Database> {
    fn database(&self) -> Arc<Database> {
        Self::clone(self)
    }
}

#[async_trait::async_trait]
impl DatabaseProvider for Database {
    fn get_postgres_pool(&self) -> Arc<sqlx::PgPool> {
        self.read().get_postgres_pool()
    }

    async fn execute(
        &self,
        query: &dyn crate::models::QuerySelector,
        params: &[&dyn crate::models::ToDbValue],
    ) -> DatabaseResult<u64> {
        self.write().execute(query, params).await
    }

    async fn execute_raw(&self, sql: &str) -> DatabaseResult<()> {
        self.write().execute_raw(sql).await
    }

    async fn fetch_all(
        &self,
        query: &dyn crate::models::QuerySelector,
        params: &[&dyn crate::models::ToDbValue],
    ) -> DatabaseResult<Vec<crate::models::JsonRow>> {
        self.read().fetch_all(query, params).await
    }

    async fn fetch_one(
        &self,
        query: &dyn crate::models::QuerySelector,
        params: &[&dyn crate::models::ToDbValue],
    ) -> DatabaseResult<crate::models::JsonRow> {
        self.read().fetch_one(query, params).await
    }

    async fn fetch_optional(
        &self,
        query: &dyn crate::models::QuerySelector,
        params: &[&dyn crate::models::ToDbValue],
    ) -> DatabaseResult<Option<crate::models::JsonRow>> {
        self.read().fetch_optional(query, params).await
    }

    async fn begin_transaction(
        &self,
    ) -> DatabaseResult<Box<dyn crate::models::DatabaseTransaction>> {
        self.write().begin_transaction().await
    }

    async fn get_database_info(&self) -> DatabaseResult<DatabaseInfo> {
        self.read().get_database_info().await
    }

    async fn test_connection(&self) -> DatabaseResult<()> {
        self.read().test_connection().await
    }

    async fn execute_batch(&self, sql: &str) -> DatabaseResult<()> {
        self.write().execute_batch(sql).await
    }

    async fn query_raw(
        &self,
        query: &dyn crate::models::QuerySelector,
    ) -> DatabaseResult<QueryResult> {
        self.read().query_raw(query).await
    }

    async fn query_raw_with(
        &self,
        query: &dyn crate::models::QuerySelector,
        params: &[&dyn crate::models::ToDbValue],
    ) -> DatabaseResult<QueryResult> {
        self.read().query_raw_with(query, params).await
    }
}
