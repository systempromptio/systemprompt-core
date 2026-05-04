//! Shared low-level helpers used by both the module-based and
//! extension-based installation pipelines.

use anyhow::Result;

use crate::services::DatabaseProvider;

pub(super) async fn table_exists(db: &dyn DatabaseProvider, table_name: &str) -> Result<bool> {
    let result = db
        .query_raw_with(
            &"SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = 'public' AND table_name = $1) as exists",
            vec![serde_json::Value::String(table_name.to_string())],
        )
        .await
        .map_err(|e| {
            tracing::error!(error = %e, table = %table_name, "Database error checking table existence");
            anyhow::anyhow!("Database error checking table '{}': {}", table_name, e)
        })?;

    let exists = result
        .rows
        .first()
        .and_then(|row| row.get("exists"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    Ok(exists)
}
