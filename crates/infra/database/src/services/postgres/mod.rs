//! `PostgreSQL` implementation of [`crate::services::DatabaseProvider`].
//!
//! This module is part of the documented sqlx allowlist: every `sqlx::query(_)`
//! call here either binds a [`crate::models::QuerySelector`] string supplied
//! at runtime (extension-defined SQL, dynamic admin queries) or executes
//! `SELECT 1` for connection probing. Static SQL goes through the verified
//! macros elsewhere.

pub mod conversion;
mod ext;
mod introspection;
pub mod transaction;

use async_trait::async_trait;
use sqlx::Executor;
use sqlx::postgres::{PgConnectOptions, PgPool, PgSslMode};
use std::str::FromStr;
use std::sync::Arc;

use super::provider::DatabaseProvider;
use crate::error::{DatabaseResult, RepositoryError};
use crate::models::{
    DatabaseInfo, DatabaseTransaction, DbValue, JsonRow, QueryResult, QuerySelector, ToDbValue,
};
use conversion::{bind_params, row_to_json, rows_to_result};
use transaction::PostgresTransaction;

#[derive(Debug)]
pub struct PostgresProvider {
    pool: Arc<PgPool>,
}

impl PostgresProvider {
    pub async fn new(database_url: &str) -> DatabaseResult<Self> {
        let mut connect_options = PgConnectOptions::from_str(database_url)?;

        let ssl_mode = if database_url.contains("sslmode=require") {
            PgSslMode::Require
        } else if database_url.contains("sslmode=disable") {
            PgSslMode::Disable
        } else {
            PgSslMode::Prefer
        };

        connect_options = connect_options
            .application_name("systemprompt")
            .statement_cache_capacity(0)
            .ssl_mode(ssl_mode)
            .options([("client_min_messages", "warning")]);

        if let Some(ca_cert_path) = Self::get_cert_path() {
            connect_options = connect_options.ssl_root_cert(&ca_cert_path);
        }

        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(50)
            .min_connections(0)
            .max_lifetime(std::time::Duration::from_secs(1800))
            .acquire_timeout(std::time::Duration::from_secs(30))
            .idle_timeout(std::time::Duration::from_secs(300))
            .connect_with(connect_options)
            .await?;

        Ok(Self {
            pool: Arc::new(pool),
        })
    }

    fn get_cert_path() -> Option<std::path::PathBuf> {
        std::env::var("PGCA_CERT_PATH")
            .ok()
            .map(std::path::PathBuf::from)
    }

    #[must_use]
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait]
impl DatabaseProvider for PostgresProvider {
    fn get_postgres_pool(&self) -> Option<Arc<PgPool>> {
        Some(Arc::clone(&self.pool))
    }

    async fn execute(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<u64> {
        let sql = query.select_query();
        let query_obj = sqlx::query(sql);
        let query_obj = bind_params(query_obj, params);

        let result = query_obj.execute(&*self.pool).await?;

        Ok(result.rows_affected())
    }

    async fn execute_raw(&self, sql: &str) -> DatabaseResult<()> {
        let mut conn = self.pool.acquire().await?;

        conn.execute(sql).await?;

        Ok(())
    }

    async fn fetch_all(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<Vec<JsonRow>> {
        let sql = query.select_query();
        let query_obj = sqlx::query(sql);
        let query_obj = bind_params(query_obj, params);

        let rows = query_obj.fetch_all(&*self.pool).await?;

        Ok(rows.iter().map(row_to_json).collect())
    }

    async fn fetch_one(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<JsonRow> {
        let sql = query.select_query();
        let query_obj = sqlx::query(sql);
        let query_obj = bind_params(query_obj, params);

        let row = query_obj.fetch_one(&*self.pool).await?;

        Ok(row_to_json(&row))
    }

    async fn fetch_optional(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<Option<JsonRow>> {
        let sql = query.select_query();
        let query_obj = sqlx::query(sql);
        let query_obj = bind_params(query_obj, params);

        let row = query_obj.fetch_optional(&*self.pool).await?;

        Ok(row.map(|r| row_to_json(&r)))
    }

    async fn fetch_scalar_value(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<DbValue> {
        let row = self.fetch_one(query, params).await?;

        let first_value = row
            .values()
            .next()
            .ok_or_else(|| RepositoryError::invalid_state("No columns in result"))?;

        let db_value = match first_value {
            serde_json::Value::String(s) => DbValue::String(s.clone()),
            serde_json::Value::Number(n) => n
                .as_i64()
                .map(DbValue::Int)
                .or_else(|| n.as_f64().map(DbValue::Float))
                .unwrap_or(DbValue::NullFloat),
            serde_json::Value::Bool(b) => DbValue::Bool(*b),
            serde_json::Value::Null => DbValue::NullString,
            serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                return Err(RepositoryError::invalid_state("Unsupported value type"));
            },
        };

        Ok(db_value)
    }

    async fn begin_transaction(&self) -> DatabaseResult<Box<dyn DatabaseTransaction>> {
        let tx = self.pool.begin().await?;

        Ok(Box::new(PostgresTransaction::new(tx)))
    }

    async fn get_database_info(&self) -> DatabaseResult<DatabaseInfo> {
        introspection::get_database_info(&self.pool).await
    }

    async fn test_connection(&self) -> DatabaseResult<()> {
        sqlx::query("SELECT 1").fetch_one(&*self.pool).await?;
        Ok(())
    }

    async fn execute_batch(&self, sql: &str) -> DatabaseResult<()> {
        let statements = crate::services::SqlExecutor::parse_sql_statements(sql);
        for statement in statements {
            sqlx::query(&statement).execute(&*self.pool).await?;
        }
        Ok(())
    }

    async fn query_raw(&self, query: &dyn QuerySelector) -> DatabaseResult<QueryResult> {
        let sql = query.select_query();
        let start = std::time::Instant::now();

        let rows = sqlx::query(sql).fetch_all(&*self.pool).await?;

        Ok(rows_to_result(rows, start))
    }

    // JSON: dynamic query params — type-erased for heterogeneous admin queries
    async fn query_raw_with(
        &self,
        query: &dyn QuerySelector,
        params: Vec<serde_json::Value>,
    ) -> DatabaseResult<QueryResult> {
        let sql = query.select_query();
        let start = std::time::Instant::now();

        let mut query_obj = sqlx::query(sql);
        for param in params {
            query_obj = match param {
                serde_json::Value::String(s) => query_obj.bind(s),
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        query_obj.bind(i)
                    } else if let Some(f) = n.as_f64() {
                        query_obj.bind(f)
                    } else {
                        query_obj.bind(None::<i64>)
                    }
                },
                serde_json::Value::Bool(b) => query_obj.bind(b),
                serde_json::Value::Null => query_obj.bind(None::<String>),
                serde_json::Value::Array(arr) => {
                    let strings: Vec<String> = arr
                        .into_iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect();
                    query_obj.bind(strings)
                },
                serde_json::Value::Object(_) => {
                    let json_str = serde_json::to_string(&param)?;
                    query_obj.bind(Some(json_str))
                },
            };
        }

        let rows = query_obj.fetch_all(&*self.pool).await?;

        Ok(rows_to_result(rows, start))
    }
}
