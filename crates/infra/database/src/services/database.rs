//! Top-level [`Database`] handle that owns one or two
//! [`DatabaseProvider`] instances (read + optional write) and exposes the
//! query and transaction surface.

use super::postgres::PostgresProvider;
use super::provider::DatabaseProvider;
use crate::error::{DatabaseResult, RepositoryError};
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

    pub async fn from_config(db_type: &str, url: &str) -> DatabaseResult<Self> {
        match db_type.to_lowercase().as_str() {
            "postgres" | "postgresql" | "" => Self::new_postgres(url).await,
            other => Err(RepositoryError::invalid_argument(format!(
                "Unsupported database type: {other}. Only PostgreSQL is supported."
            ))),
        }
    }

    pub async fn from_config_with_write(
        db_type: &str,
        read_url: &str,
        write_url: Option<&str>,
    ) -> DatabaseResult<Self> {
        let provider: Arc<dyn DatabaseProvider> = match db_type.to_lowercase().as_str() {
            "postgres" | "postgresql" | "" => Arc::new(PostgresProvider::new(read_url).await?),
            other => {
                return Err(RepositoryError::invalid_argument(format!(
                    "Unsupported database type: {other}. Only PostgreSQL is supported."
                )));
            },
        };

        let write_provider: Option<Arc<dyn DatabaseProvider>> = match write_url {
            Some(url) => Some(Arc::new(PostgresProvider::new(url).await?)),
            None => None,
        };

        Ok(Self {
            provider,
            write_provider,
        })
    }

    /// Builds a handle from pools the caller already holds, reusing the open
    /// connections rather than dialing the database again. The intended caller
    /// is an extension HTTP router that is handed an `Arc<PgPool>` and needs to
    /// construct core data services (which require a `Database`) without a URL.
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

    fn require_postgres(pool: Option<Arc<sqlx::PgPool>>) -> DatabaseResult<Arc<sqlx::PgPool>> {
        pool.ok_or_else(|| RepositoryError::invalid_state("Database is not PostgreSQL"))
    }

    /// Provider that serves reads. Equal to [`Self::write`] when no separate
    /// write URL is configured (single-node deployments).
    #[must_use]
    pub fn read(&self) -> &dyn DatabaseProvider {
        self.provider.as_ref()
    }

    /// Provider that serves writes and transactions. Falls back to the read
    /// provider when no separate write URL is configured.
    #[must_use]
    pub fn write(&self) -> &dyn DatabaseProvider {
        self.write_provider
            .as_deref()
            .unwrap_or_else(|| self.provider.as_ref())
    }

    #[must_use]
    pub fn pool(&self) -> Option<Arc<sqlx::PgPool>> {
        self.read().get_postgres_pool()
    }

    pub fn pool_arc(&self) -> DatabaseResult<Arc<sqlx::PgPool>> {
        Self::require_postgres(self.read().get_postgres_pool())
    }

    #[must_use]
    pub fn write_pool(&self) -> Option<Arc<sqlx::PgPool>> {
        self.write().get_postgres_pool()
    }

    pub fn write_pool_arc(&self) -> DatabaseResult<Arc<sqlx::PgPool>> {
        Self::require_postgres(self.write().get_postgres_pool())
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
        let pool = self.write_pool_arc()?;
        pool.begin().await.map_err(Into::into)
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
    fn get_postgres_pool(&self) -> Option<Arc<sqlx::PgPool>> {
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

    async fn fetch_scalar_value(
        &self,
        query: &dyn crate::models::QuerySelector,
        params: &[&dyn crate::models::ToDbValue],
    ) -> DatabaseResult<crate::models::DbValue> {
        self.read().fetch_scalar_value(query, params).await
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
