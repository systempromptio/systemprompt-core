//! SQL batch and statement-by-statement execution helpers.

use super::database::Database;
use super::provider::DatabaseProvider;
use crate::error::{DatabaseResult, RepositoryError};
use crate::models::QueryResult;

#[derive(Debug, Copy, Clone)]
pub struct SqlExecutor;

impl SqlExecutor {
    pub async fn execute_statements(db: &Database, sql: &str) -> DatabaseResult<()> {
        db.execute_batch(sql).await.map_err(|e| {
            RepositoryError::Internal(format!("Failed to execute SQL statements: {e}"))
        })
    }

    pub async fn execute_statements_parsed(
        db: &dyn DatabaseProvider,
        sql: &str,
    ) -> DatabaseResult<()> {
        let statements = Self::parse_sql_statements(sql)?;

        for statement in statements {
            db.execute_raw(&statement).await.map_err(|e| {
                RepositoryError::Internal(format!(
                    "Failed to execute SQL statement: {statement}: {e}"
                ))
            })?;
        }

        Ok(())
    }

    pub fn parse_sql_statements(sql: &str) -> DatabaseResult<Vec<String>> {
        use sqlparser::dialect::PostgreSqlDialect;
        use sqlparser::parser::Parser;

        Parser::parse_sql(&PostgreSqlDialect {}, sql)
            .map_err(|e| RepositoryError::Internal(format!("SQL parse failed: {e}")))
            .map(|stmts| stmts.into_iter().map(|s| s.to_string()).collect())
    }

    pub async fn execute_query(db: &Database, query: &str) -> DatabaseResult<QueryResult> {
        db.query(&query)
            .await
            .map_err(|e| RepositoryError::Internal(format!("Failed to execute query: {e}")))
    }

    pub async fn execute_file(db: &Database, file_path: &str) -> DatabaseResult<()> {
        let sql = std::fs::read_to_string(file_path).map_err(|e| {
            RepositoryError::Internal(format!("Failed to read SQL file: {file_path}: {e}"))
        })?;
        Self::execute_statements(db, &sql).await
    }

    pub async fn execute_file_parsed(
        db: &dyn DatabaseProvider,
        file_path: &str,
    ) -> DatabaseResult<()> {
        let sql = std::fs::read_to_string(file_path).map_err(|e| {
            RepositoryError::Internal(format!("Failed to read SQL file: {file_path}: {e}"))
        })?;
        Self::execute_statements_parsed(db, &sql).await
    }

    pub async fn table_exists(db: &Database, table_name: &str) -> DatabaseResult<bool> {
        let result = db
            .query_with(
                &"SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = \
                  'public' AND table_name = $1) as exists",
                vec![serde_json::Value::String(table_name.to_string())],
            )
            .await?;

        result
            .first()
            .and_then(|row| row.get("exists"))
            .and_then(serde_json::Value::as_bool)
            .ok_or_else(|| RepositoryError::Internal("Failed to check table existence".to_string()))
    }

    pub async fn column_exists(
        db: &Database,
        table_name: &str,
        column_name: &str,
    ) -> DatabaseResult<bool> {
        let result = db
            .query_with(
                &"SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema = \
                  'public' AND table_name = $1 AND column_name = $2) as exists",
                vec![
                    serde_json::Value::String(table_name.to_string()),
                    serde_json::Value::String(column_name.to_string()),
                ],
            )
            .await?;

        result
            .first()
            .and_then(|row| row.get("exists"))
            .and_then(serde_json::Value::as_bool)
            .ok_or_else(|| {
                RepositoryError::Internal("Failed to check column existence".to_string())
            })
    }
}
