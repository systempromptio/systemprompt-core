use std::env;
use std::sync::Arc;

use anyhow::{Context, Result};
use systemprompt_database::Database;

#[tokio::main]
async fn main() -> Result<()> {
    let url = env::var("DATABASE_URL")
        .context("DATABASE_URL must be set for systemprompt-test-migrate")?;

    println!("Connecting to {}", mask_password(&url));
    let db = Arc::new(
        Database::new_postgres(&url)
            .await
            .context("Failed to connect to Postgres")?,
    );

    println!(
        "Installing {} core members",
        systemprompt_test_migrate::core_members().len()
    );
    systemprompt_test_migrate::install_core_schemas(db.write())
        .await
        .context("Schema installation failed")?;

    println!("Extension schemas applied.");
    Ok(())
}

fn mask_password(url: &str) -> String {
    if let Some(at_idx) = url.rfind('@')
        && let Some(scheme_end) = url.find("://")
    {
        let head = &url[..scheme_end + 3];
        let tail = &url[at_idx..];
        return format!("{head}***{tail}");
    }
    url.to_string()
}
