//! Typed-row extension methods over [`PostgresProvider`].
//!
//! Part of the documented sqlx allowlist: the SQL is supplied dynamically
//! through [`QuerySelector`], so compile-time verification is impossible.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::DatabaseResult;

use super::PostgresProvider;
use super::conversion::bind_params;
use crate::models::{FromDatabaseRow, QuerySelector, ToDbValue};
use crate::services::provider::DatabaseProviderExt;

impl DatabaseProviderExt for PostgresProvider {
    async fn fetch_typed_optional<T: FromDatabaseRow>(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<Option<T>> {
        let sql = query.select_query();
        let query_obj = sqlx::query(sqlx::AssertSqlSafe(sql));
        let query_obj = bind_params(query_obj, params);

        let row = query_obj.fetch_optional(self.pool()).await?;

        match row {
            Some(r) => Ok(Some(T::from_postgres_row(&r)?)),
            None => Ok(None),
        }
    }

    async fn fetch_typed_one<T: FromDatabaseRow>(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<T> {
        let sql = query.select_query();
        let query_obj = sqlx::query(sqlx::AssertSqlSafe(sql));
        let query_obj = bind_params(query_obj, params);

        let row = query_obj.fetch_one(self.pool()).await?;

        T::from_postgres_row(&row)
    }

    async fn fetch_typed_all<T: FromDatabaseRow>(
        &self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<Vec<T>> {
        let sql = query.select_query();
        let query_obj = sqlx::query(sqlx::AssertSqlSafe(sql));
        let query_obj = bind_params(query_obj, params);

        let rows = query_obj.fetch_all(self.pool()).await?;

        rows.iter().map(|r| T::from_postgres_row(r)).collect()
    }
}
