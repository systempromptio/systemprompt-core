use std::sync::Arc;

use anyhow::Result;
use sqlx::postgres::PgPool;
use sqlx::Row;

use crate::models::{ColumnInfo, DatabaseInfo, IndexInfo, TableInfo};

#[derive(Debug)]
pub struct DatabaseAdminService {
    pool: Arc<PgPool>,
}

impl DatabaseAdminService {
    pub const fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    pub async fn list_tables(&self) -> Result<Vec<TableInfo>> {
        let rows = sqlx::query(
            r"
            SELECT
                t.table_name as name,
                COALESCE(s.n_live_tup, 0) as row_count,
                COALESCE(pg_total_relation_size(quote_ident(t.table_name)::regclass), 0) as size_bytes
            FROM information_schema.tables t
            LEFT JOIN pg_stat_user_tables s ON t.table_name = s.relname
            WHERE t.table_schema = 'public'
            ORDER BY t.table_name
            ",
        )
        .fetch_all(&*self.pool)
        .await?;

        let tables = rows
            .iter()
            .map(|row| {
                let name: String = row.get("name");
                let row_count: i64 = row.get("row_count");
                let size_bytes: i64 = row.get("size_bytes");
                TableInfo {
                    name,
                    row_count,
                    size_bytes,
                    columns: vec![],
                }
            })
            .collect();

        Ok(tables)
    }

    pub async fn describe_table(&self, table_name: &str) -> Result<(Vec<ColumnInfo>, i64)> {
        if !table_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(anyhow::anyhow!("Table '{}' not found", table_name));
        }

        let rows = sqlx::query(
            "SELECT column_name, data_type, is_nullable, column_default FROM \
             information_schema.columns WHERE table_name = $1 ORDER BY ordinal_position",
        )
        .bind(table_name)
        .fetch_all(&*self.pool)
        .await?;

        if rows.is_empty() {
            return Err(anyhow::anyhow!("Table '{}' not found", table_name));
        }

        let pk_rows = sqlx::query(
            r"
            SELECT a.attname as column_name
            FROM pg_index i
            JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey)
            WHERE i.indrelid = $1::regclass AND i.indisprimary
            ",
        )
        .bind(table_name)
        .fetch_all(&*self.pool)
        .await
        .unwrap_or_else(|_| Vec::new());

        let pk_columns: Vec<String> = pk_rows
            .iter()
            .map(|row| row.get::<String, _>("column_name"))
            .collect();

        let columns = rows
            .iter()
            .map(|row| {
                let name: String = row.get("column_name");
                let data_type: String = row.get("data_type");
                let nullable_str: String = row.get("is_nullable");
                let nullable = nullable_str.to_uppercase() == "YES";
                let default: Option<String> = row.get("column_default");
                let primary_key = pk_columns.contains(&name);

                ColumnInfo {
                    name,
                    data_type,
                    nullable,
                    primary_key,
                    default,
                }
            })
            .collect();

        let row_count = self.count_rows(table_name).await?;

        Ok((columns, row_count))
    }

    pub async fn get_table_indexes(&self, table_name: &str) -> Result<Vec<IndexInfo>> {
        if !table_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(anyhow::anyhow!("Table '{}' not found", table_name));
        }

        let rows = sqlx::query(
            r"
            SELECT
                i.relname as index_name,
                ix.indisunique as is_unique,
                array_agg(a.attname ORDER BY array_position(ix.indkey, a.attnum)) as columns
            FROM pg_class t
            JOIN pg_index ix ON t.oid = ix.indrelid
            JOIN pg_class i ON i.oid = ix.indexrelid
            JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = ANY(ix.indkey)
            WHERE t.relname = $1 AND t.relkind = 'r'
            GROUP BY i.relname, ix.indisunique
            ORDER BY i.relname
            ",
        )
        .bind(table_name)
        .fetch_all(&*self.pool)
        .await?;

        let indexes = rows
            .iter()
            .map(|row| {
                let name: String = row.get("index_name");
                let unique: bool = row.get("is_unique");
                let columns: Vec<String> = row.get("columns");
                IndexInfo {
                    name,
                    columns,
                    unique,
                }
            })
            .collect();

        Ok(indexes)
    }

    pub async fn count_rows(&self, table_name: &str) -> Result<i64> {
        if !table_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(anyhow::anyhow!("Table '{}' not found", table_name));
        }

        let quoted_table = quote_identifier(table_name);
        let count_query = format!("SELECT COUNT(*) as count FROM {quoted_table}");
        let row_count: i64 = sqlx::query_scalar(&count_query)
            .fetch_one(&*self.pool)
            .await?;

        Ok(row_count)
    }

    pub async fn get_database_info(&self) -> Result<DatabaseInfo> {
        let version: String = sqlx::query_scalar("SELECT version()")
            .fetch_one(&*self.pool)
            .await?;

        let size: i64 = sqlx::query_scalar("SELECT pg_database_size(current_database())")
            .fetch_one(&*self.pool)
            .await?;

        let tables = self.list_tables().await?;

        Ok(DatabaseInfo {
            path: "PostgreSQL".to_string(),
            size: u64::try_from(size).unwrap_or(0),
            version,
            tables,
        })
    }

    pub fn get_expected_tables() -> Vec<&'static str> {
        vec![
            "users",
            "user_sessions",
            "user_contexts",
            "agent_tasks",
            "agent_skills",
            "task_messages",
            "task_artifacts",
            "task_execution_steps",
            "artifact_parts",
            "message_parts",
            "ai_requests",
            "ai_request_messages",
            "ai_request_tool_calls",
            "mcp_tool_executions",
            "logs",
            "analytics_events",
            "oauth_clients",
            "oauth_auth_codes",
            "oauth_refresh_tokens",
            "scheduled_jobs",
            "services",
            "markdown_content",
            "markdown_categories",
            "files",
            "content_files",
        ]
    }
}

fn quote_identifier(identifier: &str) -> String {
    let escaped = identifier.replace('"', "\"\"");
    format!("\"{escaped}\"")
}
