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
        let statements = Self::parse_sql_statements(sql);

        for statement in statements {
            db.execute_raw(&statement).await.map_err(|e| {
                RepositoryError::Internal(format!(
                    "Failed to execute SQL statement: {statement}: {e}"
                ))
            })?;
        }

        Ok(())
    }

    pub fn parse_sql_statements(sql: &str) -> Vec<String> {
        let mut statements = Vec::new();
        let mut current_statement = String::new();
        let mut in_trigger = false;
        let mut in_dollar_quote = false;
        let mut dollar_count = 0;

        for line in sql.lines() {
            let trimmed = line.trim();

            if Self::should_skip_line(trimmed) {
                continue;
            }

            current_statement.push_str(line);
            current_statement.push('\n');

            if trimmed.contains("$$") {
                dollar_count += trimmed.matches("$$").count();
                in_dollar_quote = dollar_count % 2 == 1;
            }

            if trimmed.starts_with("CREATE TRIGGER")
                || trimmed.starts_with("CREATE OR REPLACE FUNCTION")
            {
                in_trigger = true;
            }

            if Self::is_statement_complete(trimmed, in_trigger, in_dollar_quote) {
                let stmt = current_statement.trim().to_string();
                if !stmt.is_empty() {
                    statements.push(stmt);
                }
                current_statement.clear();
                in_trigger = false;
                dollar_count = 0;
            }
        }

        let stmt = current_statement.trim().to_string();
        if !stmt.is_empty() {
            statements.push(stmt);
        }

        statements
    }

    fn should_skip_line(line: &str) -> bool {
        line.starts_with("--") || line.is_empty()
    }

    fn is_statement_complete(line: &str, in_trigger: bool, in_dollar_quote: bool) -> bool {
        if in_dollar_quote {
            return false;
        }

        if in_trigger {
            return line == "END;" || line.ends_with("LANGUAGE plpgsql;");
        }

        line.ends_with(';')
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
