//! Pre-flight validation helpers used by the boot path and tests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::{DatabaseResult, RepositoryError};
use crate::services::{Database, DatabaseProvider};

pub async fn validate_database_connection(db: &dyn DatabaseProvider) -> DatabaseResult<()> {
    db.test_connection().await.map_err(|e| {
        RepositoryError::Internal(format!("Failed to establish database connection: {e}"))
    })
}

/// Rejects a write pool that resolves to a streaming-replication standby.
///
/// Every write path — schema installation, extension seeds, the job scheduler,
/// log persistence, and `LISTEN`/`NOTIFY` on the event bridge — goes through
/// [`Database::write`], which falls back to the read pool when no separate write
/// URL is configured. A read URL aimed at a replica therefore turns into a slow,
/// opaque boot failure: DDL stalls, then jobs die on `25006`. Failing here names
/// the cause instead.
pub async fn validate_write_pool_is_primary(db: &Database) -> DatabaseResult<()> {
    if !db.write().is_postgres() {
        return Ok(());
    }

    let result = db
        .write()
        .query_raw(&"SELECT pg_is_in_recovery() as in_recovery")
        .await?;

    let in_recovery = result
        .first()
        .and_then(|row| row.get("in_recovery"))
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| {
            RepositoryError::Internal(
                "Failed to determine whether the write pool is a primary".to_owned(),
            )
        })?;

    if !in_recovery {
        return Ok(());
    }

    Err(RepositoryError::invalid_state(if db.has_write_pool() {
        "`database_write_url` points at a read-only standby. Writes, migrations and \
         LISTEN/NOTIFY all require the primary — point it at the primary and restart"
    } else {
        "`database_url` points at a read-only standby and no `database_write_url` is set, so \
         the write pool falls back to it. Set `database_write_url` (or `DATABASE_WRITE_URL` \
         with the env secrets source) to the primary and restart"
    }))
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
