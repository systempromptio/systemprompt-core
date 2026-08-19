//! Post-create profile setup, including the Postgres compose container.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use std::path::Path;
use systemprompt_cloud::{DockerCli, ProjectContext};
use systemprompt_logging::CliService;

use super::templates::{run_migrations_cmd, validate_connection};
use crate::cloud::tenant::wait_for_postgres_healthy;
use crate::interactive::Prompter;

pub async fn handle_local_tenant_setup(
    prompter: &dyn Prompter,
    db_url: &str,
    tenant_name: &str,
    profile_path: &Path,
) -> Result<()> {
    let spinner = CliService::spinner("Validating PostgreSQL connection...");
    let mut connection_valid = validate_connection(db_url).await;
    spinner.finish_and_clear();

    if !connection_valid {
        let ctx = ProjectContext::discover();
        let compose_path = ctx.docker_dir().join(format!("{}.yaml", tenant_name));

        if compose_path.exists() {
            let start_docker =
                prompter.confirm("PostgreSQL not running. Start Docker container?", true)?;

            if start_docker {
                connection_valid = start_postgres_container(&compose_path).await?;
            }
        } else {
            CliService::warning("Could not connect to PostgreSQL.");
            CliService::info("Ensure PostgreSQL is running before starting services.");
        }
    }

    if connection_valid {
        CliService::success("PostgreSQL connection verified");

        let run_migrations = prompter.confirm("Run database migrations?", true)?;

        let migrations_succeeded = if run_migrations {
            match run_migrations_cmd(profile_path) {
                Ok(()) => true,
                Err(e) => {
                    CliService::warning(&format!("Migration failed: {}", e));
                    false
                },
            }
        } else {
            false
        };

        if migrations_succeeded {
            CliService::info(
                "Run 'systemprompt admin bootstrap' to ensure the profile's system-admin user \
                 exists.",
            );
        }
    }

    Ok(())
}

async fn start_postgres_container(compose_path: &Path) -> Result<bool> {
    CliService::info("Starting PostgreSQL container...");

    let compose_path_str = compose_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid compose path"))?;
    let project = compose_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid compose path"))?
        .to_owned();

    let docker = DockerCli::new();
    let status = docker
        .status(&[
            "compose",
            "-p",
            &project,
            "-f",
            compose_path_str,
            "up",
            "-d",
        ])
        .map_err(|_e| anyhow::anyhow!("Failed to execute docker compose. Is Docker running?"))?;

    if !status.success() {
        CliService::warning("Failed to start PostgreSQL container. Is Docker running?");
        return Ok(false);
    }

    let spinner = CliService::spinner("Waiting for PostgreSQL to be ready...");
    match wait_for_postgres_healthy(&docker, &project, compose_path, 60).await {
        Ok(()) => {
            spinner.finish_and_clear();
            Ok(true)
        },
        Err(e) => {
            spinner.finish_and_clear();
            CliService::warning(&format!("PostgreSQL failed to become healthy: {}", e));
            Ok(false)
        },
    }
}
