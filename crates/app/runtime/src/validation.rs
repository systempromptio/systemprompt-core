//! System pre-flight validation: database URL shape and connectivity.

use crate::AppContext;
use crate::error::{RuntimeError, RuntimeResult};
use std::path::Path;
use systemprompt_database::validate_database_connection;

/// Validate end-to-end system readiness against `ctx`.
///
/// Currently this checks the database URL shape and confirms the pool
/// can round-trip a `SELECT 1`. Additional preflight checks may be
/// added here over time.
pub async fn validate_system(ctx: &AppContext) -> RuntimeResult<()> {
    validate_database(ctx).await
}

async fn validate_database(ctx: &AppContext) -> RuntimeResult<()> {
    validate_database_path(&ctx.config().database_url)?;
    validate_database_connection(ctx.db_pool().as_ref()).await?;
    Ok(())
}

/// Validate that `db_path` is a usable database URL.
///
/// `Postgres` URLs are accepted as-is. `SQLite` paths must point at an
/// existing regular file.
pub fn validate_database_path(db_path: &str) -> RuntimeResult<()> {
    if db_path.is_empty() {
        return Err(RuntimeError::EmptyDatabaseUrl);
    }

    if db_path.starts_with("postgresql://") || db_path.starts_with("postgres://") {
        return Ok(());
    }

    let path = Path::new(db_path);

    if !path.exists() {
        return Err(RuntimeError::DatabaseNotFound {
            path: db_path.to_string(),
        });
    }

    if !path.is_file() {
        return Err(RuntimeError::DatabaseNotFile {
            path: db_path.to_string(),
        });
    }

    Ok(())
}
