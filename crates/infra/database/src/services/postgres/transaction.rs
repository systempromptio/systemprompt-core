//! Concrete [`DatabaseTransaction`] for `PostgreSQL`.
//!
//! Part of the documented sqlx allowlist — the SQL strings here come from
//! runtime-supplied [`QuerySelector`] values.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::{DatabaseResult, RepositoryError};
use async_trait::async_trait;

use super::conversion::{bind_params, row_to_json};
use crate::models::{DatabaseTransaction, JsonRow, QuerySelector, ToDbValue};

pub struct PostgresTransaction {
    tx: Option<sqlx::Transaction<'static, sqlx::Postgres>>,
}

impl std::fmt::Debug for PostgresTransaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostgresTransaction")
            .field("tx", &self.tx.is_some())
            .finish()
    }
}

impl PostgresTransaction {
    #[must_use]
    pub const fn new(tx: sqlx::Transaction<'static, sqlx::Postgres>) -> Self {
        Self { tx: Some(tx) }
    }
}

#[async_trait]
impl DatabaseTransaction for PostgresTransaction {
    async fn execute(
        &mut self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<u64> {
        let sql = query.select_query();
        let tx = self
            .tx
            .as_mut()
            .ok_or_else(|| RepositoryError::invalid_state("Transaction already consumed"))?;

        let query_obj = sqlx::query(sqlx::AssertSqlSafe(sql));
        let query_obj = bind_params(query_obj, params);

        let result = query_obj.execute(&mut **tx).await?;

        Ok(result.rows_affected())
    }

    async fn fetch_all(
        &mut self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<Vec<JsonRow>> {
        let sql = query.select_query();
        let tx = self
            .tx
            .as_mut()
            .ok_or_else(|| RepositoryError::invalid_state("Transaction already consumed"))?;

        let query_obj = sqlx::query(sqlx::AssertSqlSafe(sql));
        let query_obj = bind_params(query_obj, params);

        let rows = query_obj.fetch_all(&mut **tx).await?;

        Ok(rows.iter().map(row_to_json).collect())
    }

    async fn fetch_one(
        &mut self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<JsonRow> {
        let sql = query.select_query();
        let tx = self
            .tx
            .as_mut()
            .ok_or_else(|| RepositoryError::invalid_state("Transaction already consumed"))?;

        let query_obj = sqlx::query(sqlx::AssertSqlSafe(sql));
        let query_obj = bind_params(query_obj, params);

        let row = query_obj.fetch_one(&mut **tx).await?;

        Ok(row_to_json(&row))
    }

    async fn fetch_optional(
        &mut self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<Option<JsonRow>> {
        let sql = query.select_query();
        let tx = self
            .tx
            .as_mut()
            .ok_or_else(|| RepositoryError::invalid_state("Transaction already consumed"))?;

        let query_obj = sqlx::query(sqlx::AssertSqlSafe(sql));
        let query_obj = bind_params(query_obj, params);

        let row = query_obj.fetch_optional(&mut **tx).await?;

        Ok(row.map(|r| row_to_json(&r)))
    }

    async fn commit(mut self: Box<Self>) -> DatabaseResult<()> {
        let tx = self
            .tx
            .take()
            .ok_or_else(|| RepositoryError::invalid_state("Transaction already consumed"))?;

        tx.commit().await?;

        Ok(())
    }

    async fn rollback(mut self: Box<Self>) -> DatabaseResult<()> {
        let tx = self
            .tx
            .take()
            .ok_or_else(|| RepositoryError::invalid_state("Transaction already consumed"))?;

        tx.rollback().await?;

        Ok(())
    }
}
