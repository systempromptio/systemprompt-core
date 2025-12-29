use std::sync::Arc;

use anyhow::Result;
use sqlx::postgres::PgPool;
use sqlx::Row;

use crate::models::{ColumnInfo, DatabaseInfo, TableInfo};

/// Service for database introspection and schema queries.
///
/// Provides safe access to database metadata like tables, columns, and row
/// counts.
#[derive(Debug)]
pub struct DatabaseAdminService {
    pool: Arc<PgPool>,
}

impl DatabaseAdminService {
    pub const fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    /// List all tables in the public schema.
    pub async fn list_tables(&self) -> Result<Vec<TableInfo>> {
        let rows = sqlx::query(
            "SELECT table_name as name FROM information_schema.tables WHERE table_schema = \
             'public' ORDER BY table_name",
        )
        .fetch_all(&*self.pool)
        .await?;

        let tables = rows
            .iter()
            .map(|row| {
                let name: String = row.get("name");
                TableInfo {
                    name,
                    row_count: 0,
                    columns: vec![],
                }
            })
            .collect();

        Ok(tables)
    }

    /// Get schema information for a specific table.
    ///
    /// Returns column definitions and row count.
    pub async fn describe_table(&self, table_name: &str) -> Result<(Vec<ColumnInfo>, i64)> {
        if !table_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(anyhow::anyhow!("Invalid table name"));
        }

        let rows = sqlx::query(
            "SELECT column_name, data_type, is_nullable, column_default FROM \
             information_schema.columns WHERE table_name = $1 ORDER BY ordinal_position",
        )
        .bind(table_name)
        .fetch_all(&*self.pool)
        .await?;

        let columns = rows
            .iter()
            .map(|row| {
                let name: String = row.get("column_name");
                let data_type: String = row.get("data_type");
                let nullable_str: String = row.get("is_nullable");
                let nullable = nullable_str.to_uppercase() == "YES";
                let default: Option<String> = row.get("column_default");

                ColumnInfo {
                    name,
                    data_type,
                    nullable,
                    default,
                    primary_key: false,
                }
            })
            .collect();

        let row_count = self.count_rows(table_name).await?;

        Ok((columns, row_count))
    }

    /// Count rows in a table.
    pub async fn count_rows(&self, table_name: &str) -> Result<i64> {
        if !table_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(anyhow::anyhow!("Invalid table name"));
        }

        let quoted_table = quote_identifier(table_name);
        let count_query = format!("SELECT COUNT(*) as count FROM {quoted_table}");
        let row_count: i64 = sqlx::query_scalar(&count_query)
            .fetch_one(&*self.pool)
            .await?;

        Ok(row_count)
    }

    /// Get database version and summary information.
    pub async fn get_database_info(&self) -> Result<DatabaseInfo> {
        let version: String = sqlx::query_scalar("SELECT version()")
            .fetch_one(&*self.pool)
            .await?;

        Ok(DatabaseInfo {
            path: "postgresql://database".to_string(),
            size: 0,
            version,
            tables: vec![],
        })
    }
}

fn quote_identifier(identifier: &str) -> String {
    let escaped = identifier.replace('"', "\"\"");
    format!("\"{escaped}\"")
}
