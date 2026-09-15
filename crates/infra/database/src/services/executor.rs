//! SQL batch and statement-by-statement execution helpers.
//!
//! Statements are split with the Postgres parser (`pg_query`), so quoted
//! identifiers, escape strings and dollar-quoted bodies never split
//! mid-token and malformed SQL is refused rather than partially executed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::database::Database;
use super::provider::DatabaseProvider;
use crate::error::{DatabaseResult, RepositoryError};
use crate::models::QueryResult;

#[derive(Debug, Copy, Clone)]
pub struct SqlExecutor;

impl SqlExecutor {
    pub async fn execute_statements(db: &Database, sql: &str) -> DatabaseResult<()> {
        db.execute_batch(sql).await
    }

    pub async fn execute_statements_parsed(
        db: &dyn DatabaseProvider,
        sql: &str,
    ) -> DatabaseResult<()> {
        for statement in Self::parse_sql_statements(sql)? {
            db.execute_raw(&statement)
                .await
                .map_err(|source| RepositoryError::Statement {
                    statement: statement.clone(),
                    source: Box::new(source),
                })?;
        }
        Ok(())
    }

    pub fn parse_sql_statements(sql: &str) -> DatabaseResult<Vec<String>> {
        let statements = pg_query::split_with_parser(sql).map_err(RepositoryError::SqlSplit)?;
        Ok(statements
            .into_iter()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect())
    }

    pub async fn execute_query(db: &Database, query: &str) -> DatabaseResult<QueryResult> {
        db.query_raw(&query)
            .await
            .map_err(|e| RepositoryError::QueryExecution(Box::new(e)))
    }

    pub async fn execute_file(db: &Database, file_path: &str) -> DatabaseResult<()> {
        let sql = tokio::fs::read_to_string(file_path)
            .await
            .map_err(|source| RepositoryError::SqlFile {
                path: file_path.to_owned(),
                source,
            })?;
        Self::execute_statements(db, &sql).await
    }
}
