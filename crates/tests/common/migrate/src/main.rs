use std::env;
use std::sync::Arc;

use anyhow::{Context, Result};
use systemprompt_database::{Database, install_extension_schemas};
use systemprompt_extension::ExtensionRegistry;

// Force the linker to keep every schema-bearing extension crate so
// their `inventory::submit!` blocks reach `ExtensionRegistry::discover()`.
// A path-only Cargo dep is not enough; rustc will drop unused crates.
#[allow(unused_imports)]
use {
    systemprompt_agent as _,
    systemprompt_ai as _,
    systemprompt_analytics as _,
    systemprompt_content as _,
    systemprompt_files as _,
    systemprompt_logging as _,
    systemprompt_mcp as _,
    systemprompt_oauth as _,
    systemprompt_scheduler as _,
    systemprompt_sync as _,
    systemprompt_users as _,
};

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

    let registry = ExtensionRegistry::discover();
    let count = registry.schema_extensions().len();
    println!("Discovered {count} schema-bearing extensions");

    install_extension_schemas(&registry, db.write_provider())
        .await
        .map_err(|e| anyhow::anyhow!("Schema installation failed: {e}"))?;

    println!("Extension schemas applied.");
    Ok(())
}

fn mask_password(url: &str) -> String {
    if let Some(at_idx) = url.rfind('@') {
        if let Some(scheme_end) = url.find("://") {
            let head = &url[..scheme_end + 3];
            let tail = &url[at_idx..];
            return format!("{head}***{tail}");
        }
    }
    url.to_string()
}
