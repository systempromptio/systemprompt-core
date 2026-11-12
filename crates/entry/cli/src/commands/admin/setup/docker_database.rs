//! Dockerised Postgres provisioning for setup.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use std::process::Command;
use systemprompt_loader::subprocess::ForegroundChildren;
use systemprompt_logging::CliService;

use super::postgres_access::{PostgresConfig, connect};
use super::provision::provision_role_and_db;

pub(super) async fn create_database_in_docker(
    children: &ForegroundChildren,
    config: &PostgresConfig,
    container_name: &str,
    reset_role_password: bool,
) -> Result<()> {
    CliService::info("Creating database and user in Docker container...");

    let mut cmd = Command::new("docker");
    cmd.args(["exec", container_name, "printenv", "POSTGRES_PASSWORD"]);
    let children = children.clone();
    let output = tokio::task::spawn_blocking(move || children.output(cmd))
        .await
        .context("Failed to get container password")?
        .context("Failed to get container password")?;

    let container_password = String::from_utf8_lossy(&output.stdout).trim().to_owned();

    if container_password.is_empty() {
        anyhow::bail!(
            "Could not get container password. The container may not have POSTGRES_PASSWORD set."
        );
    }

    let pool = connect(config.superuser_options("postgres", &container_password))
        .await
        .context("Failed to connect to Docker PostgreSQL")?;
    let outcome = provision_role_and_db(&pool, config, reset_role_password).await;
    pool.close().await;
    super::provision::report(config, outcome?);
    Ok(())
}
