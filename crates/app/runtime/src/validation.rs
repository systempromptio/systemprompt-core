//! System pre-flight validation: database URL shape and connectivity.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::AppContext;
use crate::error::{RuntimeError, RuntimeResult};
use systemprompt_database::validate_database_connection;

pub async fn validate_system(ctx: &AppContext) -> RuntimeResult<()> {
    validate_database(ctx).await
}

async fn validate_database(ctx: &AppContext) -> RuntimeResult<()> {
    validate_database_url(&ctx.config().database_url)?;
    validate_database_connection(ctx.db_pool().as_ref()).await?;
    Ok(())
}

pub fn validate_database_url(database_url: &str) -> RuntimeResult<()> {
    if database_url.is_empty() {
        return Err(RuntimeError::EmptyDatabaseUrl);
    }
    if database_url.starts_with("postgresql://") || database_url.starts_with("postgres://") {
        return Ok(());
    }
    Err(RuntimeError::UnsupportedDatabaseUrl)
}
