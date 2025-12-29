use crate::services::DatabaseProvider;
use anyhow::{Context, Result};

/// Validate that a database connection can be established
pub async fn validate_database_connection(db: &dyn DatabaseProvider) -> Result<()> {
    db.test_connection()
        .await
        .context("Failed to establish database connection")
}

/// Validate that a table exists in the database
pub async fn validate_table_exists(db: &dyn DatabaseProvider, table_name: &str) -> Result<bool> {
    let result = db
        .query_raw_with(
            &"SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = \
              'public' AND table_name = $1) as exists",
            vec![serde_json::Value::String(table_name.to_string())],
        )
        .await?;

    result
        .first()
        .and_then(|row| row.get("exists"))
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| anyhow::anyhow!("Failed to check table existence for '{}'", table_name))
}

/// Validate that a column exists in a table
pub async fn validate_column_exists(
    db: &dyn DatabaseProvider,
    table_name: &str,
    column_name: &str,
) -> Result<bool> {
    let result = db
        .query_raw_with(
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
            anyhow::anyhow!(
                "Failed to check column existence for '{}.{}'",
                table_name,
                column_name
            )
        })
}
