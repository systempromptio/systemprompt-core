//! Pre-flight validation helpers used by the boot path and tests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::{DatabaseResult, RepositoryError};
use crate::services::DatabaseProvider;

pub async fn validate_database_connection(db: &dyn DatabaseProvider) -> DatabaseResult<()> {
    db.test_connection().await.map_err(|e| {
        RepositoryError::Internal(format!("Failed to establish database connection: {e}"))
    })
}

pub async fn validate_table_exists(
    db: &dyn DatabaseProvider,
    table_name: &str,
) -> DatabaseResult<bool> {
    let result = db
        .query_raw_with(
            &"SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = \
              'public' AND table_name = $1) as exists",
            &[&table_name],
        )
        .await?;

    result
        .first()
        .and_then(|row| row.get("exists"))
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| {
            RepositoryError::Internal(format!(
                "Failed to check table existence for '{table_name}'"
            ))
        })
}

pub async fn validate_column_exists(
    db: &dyn DatabaseProvider,
    table_name: &str,
    column_name: &str,
) -> DatabaseResult<bool> {
    let result = db
        .query_raw_with(
            &"SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema = \
              'public' AND table_name = $1 AND column_name = $2) as exists",
            &[&table_name, &column_name],
        )
        .await?;

    result
        .first()
        .and_then(|row| row.get("exists"))
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| {
            RepositoryError::Internal(format!(
                "Failed to check column existence for '{table_name}.{column_name}'"
            ))
        })
}
