use crate::AppContext;
use anyhow::{bail, Result};
use std::path::Path;
use systemprompt_database::validate_database_connection;

pub async fn validate_system(ctx: &AppContext) -> Result<()> {
    validate_database(ctx).await?;
    Ok(())
}

async fn validate_database(ctx: &AppContext) -> Result<()> {
    validate_database_path(&ctx.config().database_url)?;
    validate_database_connection(ctx.db_pool().as_ref()).await?;
    Ok(())
}

fn validate_database_path(db_path: &str) -> Result<()> {
    if db_path.is_empty() {
        bail!("DATABASE_URL is empty");
    }

    if db_path.starts_with("postgresql://") || db_path.starts_with("postgres://") {
        return Ok(());
    }

    let path = Path::new(db_path);

    if !path.exists() {
        bail!("Database not found at '{db_path}'. Run setup first");
    }

    if !path.is_file() {
        bail!("Database path '{db_path}' exists but is not a file");
    }

    Ok(())
}
