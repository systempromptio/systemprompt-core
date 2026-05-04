//! Top-level [`Database`] handle that owns one or two
//! [`DatabaseProvider`] instances (read + optional write) and exposes the
//! query/transaction surface used by every repository in the workspace.

use super::postgres::PostgresProvider;
use super::provider::DatabaseProvider;
use crate::models::{DatabaseInfo, QueryResult};
use anyhow::Result;
use std::sync::Arc;

/// Owned database handle. Wraps a read provider and an optional separate
/// write provider so deployments can split reads onto a replica without
/// teaching every repository about pool selection.
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
    /// Open a single `PostgreSQL` pool and use it for both reads and writes.
    pub async fn new_postgres(url: &str) -> Result<Self> {
        let provider = PostgresProvider::new(url).await?;
        Ok(Self {
            provider: Arc::new(provider),
            write_provider: None,
        })
    }

    /// Open a database from a profile config. Currently only `postgres` is
    /// supported.
    pub async fn from_config(db_type: &str, url: &str) -> Result<Self> {
        match db_type.to_lowercase().as_str() {
            "postgres" | "postgresql" | "" => Self::new_postgres(url).await,
            other => Err(anyhow::anyhow!(
                "Unsupported database type: {other}. Only PostgreSQL is supported."
            )),
        }
    }

    /// Open a database from a profile config with separate read and write
    /// URLs. Pass `None` for `write_url` to share the read pool for writes.
    pub async fn from_config_with_write(
        db_type: &str,
        read_url: &str,
        write_url: Option<&str>,
    ) -> Result<Self> {
        let provider: Arc<dyn DatabaseProvider> = match db_type.to_lowercase().as_str() {
            "postgres" | "postgresql" | "" => Arc::new(PostgresProvider::new(read_url).await?),
            other => {
                return Err(anyhow::anyhow!(
                    "Unsupported database type: {other}. Only PostgreSQL is supported."
                ));
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

    /// Borrow the underlying `PostgreSQL` pool for low-level work.
    pub fn get_postgres_pool_arc(&self) -> Result<Arc<sqlx::PgPool>> {
        self.pool_arc()
    }

    /// Borrow the write pool, falling back to the read pool when no separate
    /// write provider was configured.
    pub fn write_pool_arc(&self) -> Result<Arc<sqlx::PgPool>> {
        self.write_provider.as_ref().map_or_else(
            || self.get_postgres_pool_arc(),
            |wp| {
                wp.get_postgres_pool()
                    .ok_or_else(|| anyhow::anyhow!("Write database is not PostgreSQL"))
            },
        )
    }

    /// Borrow the write pool if the database is `PostgreSQL`, returning
    /// `None` otherwise.
    #[must_use]
    pub fn write_pool(&self) -> Option<Arc<sqlx::PgPool>> {
        self.write_provider
            .as_ref()
            .and_then(|wp| wp.get_postgres_pool())
            .or_else(|| self.provider.get_postgres_pool())
    }

    /// Whether a separate write provider is configured.
    #[must_use]
    pub fn has_write_pool(&self) -> bool {
        self.write_provider.is_some()
    }

    /// Borrow the [`DatabaseProvider`] used for writes (falls back to the
    /// read provider when no write provider is configured).
    #[must_use]
    pub fn write_provider(&self) -> &dyn DatabaseProvider {
        self.write_provider
            .as_deref()
            .unwrap_or_else(|| self.provider.as_ref())
    }

    /// Run a parameter-less query through the read provider and return the
    /// dynamic [`QueryResult`].
    pub async fn query(&self, sql: &dyn crate::models::QuerySelector) -> Result<QueryResult> {
        self.provider.query_raw(sql).await
    }

    /// Run a query with JSON-typed dynamic parameters through the read
    /// provider.
    pub async fn query_with(
        &self,
        sql: &dyn crate::models::QuerySelector,
        params: Vec<serde_json::Value>,
    ) -> Result<QueryResult> {
        self.provider.query_raw_with(sql, params).await
    }

    /// Execute a multi-statement SQL batch through the write provider.
    pub async fn execute_batch(&self, sql: &str) -> Result<()> {
        self.provider.execute_batch(sql).await
    }

    /// Return version, table list, and database size.
    pub async fn get_info(&self) -> Result<DatabaseInfo> {
        self.provider.get_database_info().await
    }

    /// Round-trip a `SELECT 1` against both providers (when split) to verify
    /// connectivity.
    pub async fn test_connection(&self) -> Result<()> {
        self.provider.test_connection().await?;
        if let Some(wp) = &self.write_provider {
            wp.test_connection().await?;
        }
        Ok(())
    }

    /// Borrow the underlying `PostgreSQL` pool for low-level work, returning
    /// `None` when the database is not `PostgreSQL`.
    #[must_use]
    pub fn get_postgres_pool(&self) -> Option<Arc<sqlx::PgPool>> {
        self.write_provider
            .as_ref()
            .and_then(|wp| wp.get_postgres_pool())
            .or_else(|| self.provider.get_postgres_pool())
    }

    /// Borrow the underlying `PostgreSQL` pool, returning an error when the
    /// database is not `PostgreSQL`.
    pub fn pool_arc(&self) -> Result<Arc<sqlx::PgPool>> {
        self.get_postgres_pool()
            .ok_or_else(|| anyhow::anyhow!("Database is not PostgreSQL"))
    }

    /// Borrow the underlying `PostgreSQL` pool, returning `None` when not
    /// `PostgreSQL`.
    #[must_use]
    pub fn pool(&self) -> Option<Arc<sqlx::PgPool>> {
        self.get_postgres_pool()
    }

    /// Borrow the read pool, if `PostgreSQL`.
    #[must_use]
    pub fn read_pool(&self) -> Option<Arc<sqlx::PgPool>> {
        self.provider.get_postgres_pool()
    }

    /// Borrow the read pool, returning an error when not `PostgreSQL`.
    pub fn read_pool_arc(&self) -> Result<Arc<sqlx::PgPool>> {
        self.provider
            .get_postgres_pool()
            .ok_or_else(|| anyhow::anyhow!("Database is not PostgreSQL"))
    }

    /// Begin a transaction against the write pool.
    pub async fn begin(&self) -> Result<sqlx::Transaction<'_, sqlx::Postgres>> {
        let pool = self.write_pool_arc()?;
        pool.begin().await.map_err(Into::into)
    }
}

/// Shared owned [`Database`] handle. Cloning is cheap (`Arc` bump).
pub type DbPool = Arc<Database>;

/// Trait implemented by anything that can yield an [`Arc<Database>`]. Used by
/// extension code that wants to be generic over the `AppContext` type.
pub trait DatabaseExt {
    /// Borrow/clone an [`Arc<Database>`].
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
        self.write_provider
            .as_ref()
            .and_then(|wp| wp.get_postgres_pool())
            .or_else(|| self.provider.get_postgres_pool())
    }

    async fn execute(
        &self,
        query: &dyn crate::models::QuerySelector,
        params: &[&dyn crate::models::ToDbValue],
    ) -> Result<u64> {
        self.write_provider().execute(query, params).await
    }

    async fn execute_raw(&self, sql: &str) -> Result<()> {
        self.write_provider().execute_raw(sql).await
    }

    async fn fetch_all(
        &self,
        query: &dyn crate::models::QuerySelector,
        params: &[&dyn crate::models::ToDbValue],
    ) -> Result<Vec<crate::models::JsonRow>> {
        self.provider.fetch_all(query, params).await
    }

    async fn fetch_one(
        &self,
        query: &dyn crate::models::QuerySelector,
        params: &[&dyn crate::models::ToDbValue],
    ) -> Result<crate::models::JsonRow> {
        self.provider.fetch_one(query, params).await
    }

    async fn fetch_optional(
        &self,
        query: &dyn crate::models::QuerySelector,
        params: &[&dyn crate::models::ToDbValue],
    ) -> Result<Option<crate::models::JsonRow>> {
        self.provider.fetch_optional(query, params).await
    }

    async fn fetch_scalar_value(
        &self,
        query: &dyn crate::models::QuerySelector,
        params: &[&dyn crate::models::ToDbValue],
    ) -> Result<crate::models::DbValue> {
        self.provider.fetch_scalar_value(query, params).await
    }

    async fn begin_transaction(&self) -> Result<Box<dyn crate::models::DatabaseTransaction>> {
        self.write_provider().begin_transaction().await
    }

    async fn get_database_info(&self) -> Result<DatabaseInfo> {
        self.provider.get_database_info().await
    }

    async fn test_connection(&self) -> Result<()> {
        self.provider.test_connection().await
    }

    async fn execute_batch(&self, sql: &str) -> Result<()> {
        self.write_provider().execute_batch(sql).await
    }

    async fn query_raw(&self, query: &dyn crate::models::QuerySelector) -> Result<QueryResult> {
        self.provider.query_raw(query).await
    }

    async fn query_raw_with(
        &self,
        query: &dyn crate::models::QuerySelector,
        params: Vec<serde_json::Value>,
    ) -> Result<QueryResult> {
        self.provider.query_raw_with(query, params).await
    }
}
