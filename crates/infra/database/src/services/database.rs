//! Top-level [`Database`] handle that owns the read provider, an optional
//! write provider and an optional session provider, and exposes the query and
//! transaction surface.
//!
//! The session provider exists for features that need a Postgres session of
//! their own rather than a transaction: `LISTEN`, session advisory locks (the
//! replica-identity claim, scheduler job locks, the migration lock) and the
//! migration runner. Pointing `database_session_url` straight at Postgres
//! lets `database_url`/`database_write_url` go through a transaction-mode
//! pooler such as `PgBouncer`. Without it, [`Database::session`] is the write
//! provider.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::postgres::PostgresProvider;
use super::postgres::connection::{PoolConfig, SchemaEpoch};
use super::provider::DatabaseProvider;
use crate::error::DatabaseResult;
use crate::models::{DatabaseInfo, QueryResult};
use std::sync::Arc;

pub struct Database {
    provider: Arc<dyn DatabaseProvider>,
    write_provider: Option<Arc<dyn DatabaseProvider>>,
    session_provider: Option<Arc<dyn DatabaseProvider>>,
    schema_epoch: SchemaEpoch,
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
        let schema_epoch = SchemaEpoch::new();
        let provider =
            PostgresProvider::new_with_pool(url, &PoolConfig::default(), &schema_epoch).await?;
        Ok(Self {
            provider: Arc::new(provider),
            write_provider: None,
            session_provider: None,
            schema_epoch,
        })
    }

    pub async fn connect(
        read_url: &str,
        write_url: Option<&str>,
        pool: &PoolConfig,
    ) -> DatabaseResult<Self> {
        let schema_epoch = SchemaEpoch::new();
        let provider: Arc<dyn DatabaseProvider> =
            Arc::new(PostgresProvider::new_with_pool(read_url, pool, &schema_epoch).await?);

        let write_provider: Option<Arc<dyn DatabaseProvider>> = match write_url {
            Some(url) => Some(Arc::new(
                PostgresProvider::new_with_pool(url, pool, &schema_epoch).await?,
            )),
            None => None,
        };

        Ok(Self {
            provider,
            write_provider,
            session_provider: None,
            schema_epoch,
        })
    }

    pub async fn with_session_url(
        mut self,
        session_url: Option<&str>,
        pool: &PoolConfig,
    ) -> DatabaseResult<Self> {
        if let Some(url) = session_url {
            self.session_provider = Some(Arc::new(
                PostgresProvider::new_with_pool(url, pool, &self.schema_epoch).await?,
            ));
        }
        Ok(self)
    }

    #[must_use]
    pub fn from_pools(read: Arc<sqlx::PgPool>, write: Option<Arc<sqlx::PgPool>>) -> Self {
        let write_provider = write.map(|pool| -> Arc<dyn DatabaseProvider> {
            Arc::new(PostgresProvider::from_pool(pool))
        });
        Self {
            provider: Arc::new(PostgresProvider::from_pool(read)),
            write_provider,
            session_provider: None,
            schema_epoch: SchemaEpoch::new(),
        }
    }

    pub fn mark_schema_changed(&self) {
        self.schema_epoch.mark_changed();
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
    pub fn session(&self) -> &dyn DatabaseProvider {
        self.session_provider
            .as_deref()
            .unwrap_or_else(|| self.write())
    }

    #[must_use]
    pub fn pool(&self) -> Arc<sqlx::PgPool> {
        self.read().get_postgres_pool()
    }

    #[must_use]
    pub fn write_pool(&self) -> Arc<sqlx::PgPool> {
        self.write().get_postgres_pool()
    }

    #[must_use]
    pub fn session_pool(&self) -> Arc<sqlx::PgPool> {
        self.session().get_postgres_pool()
    }

    #[must_use]
    pub fn has_session_pool(&self) -> bool {
        self.session_provider.is_some()
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
        if let Some(sp) = &self.session_provider {
            sp.test_connection().await?;
        }
        Ok(())
    }

    pub async fn begin(&self) -> DatabaseResult<sqlx::Transaction<'_, sqlx::Postgres>> {
        self.write_pool().begin().await.map_err(Into::into)
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
