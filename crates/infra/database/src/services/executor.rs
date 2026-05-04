//! SQL batch and statement-by-statement execution helpers.

use super::database::Database;
use super::provider::DatabaseProvider;
use crate::models::QueryResult;
use anyhow::{Context, Result};

/// Helper namespace bundling SQL parsing and execution utilities.
#[derive(Debug, Copy, Clone)]
pub struct SqlExecutor;

impl SqlExecutor {
    /// Run an arbitrary multi-statement SQL string against `db` using the
    /// underlying provider's batch path.
    pub async fn execute_statements(db: &Database, sql: &str) -> Result<()> {
        db.execute_batch(sql)
            .await
            .context("Failed to execute SQL statements")
    }

    /// Parse `sql` into individual statements and run each one against `db`.
    /// Used when the backend's batch path cannot handle the requested DDL
    /// dialect (e.g. trigger bodies).
    pub async fn execute_statements_parsed(db: &dyn DatabaseProvider, sql: &str) -> Result<()> {
        let statements = Self::parse_sql_statements(sql);

        for statement in statements {
            db.execute_raw(&statement)
                .await
                .with_context(|| format!("Failed to execute SQL statement: {statement}"))?;
        }

        Ok(())
    }

    /// Split a SQL string into top-level statements, respecting trigger
    /// bodies and dollar-quoted blocks. Comment-only and blank lines are
    /// dropped.
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

    /// Run `query` and return the dynamic [`QueryResult`].
    pub async fn execute_query(db: &Database, query: &str) -> Result<QueryResult> {
        db.query(&query).await.context("Failed to execute query")
    }

    /// Read a SQL file from disk and run it as a batch.
    pub async fn execute_file(db: &Database, file_path: &str) -> Result<()> {
        let sql = std::fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read SQL file: {file_path}"))?;
        Self::execute_statements(db, &sql).await
    }

    /// Read a SQL file from disk and run it through the parsed-statement path.
    pub async fn execute_file_parsed(db: &dyn DatabaseProvider, file_path: &str) -> Result<()> {
        let sql = std::fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read SQL file: {file_path}"))?;
        Self::execute_statements_parsed(db, &sql).await
    }

    /// Check whether a public-schema table exists.
    pub async fn table_exists(db: &Database, table_name: &str) -> Result<bool> {
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
            .ok_or_else(|| anyhow::anyhow!("Failed to check table existence"))
    }

    /// Check whether a public-schema column exists.
    pub async fn column_exists(db: &Database, table_name: &str, column_name: &str) -> Result<bool> {
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
            .ok_or_else(|| anyhow::anyhow!("Failed to check column existence"))
    }
}
