use anyhow::Result;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use std::path::Path;
use std::process::Command;
use systemprompt_cloud::ProjectContext;
use systemprompt_logging::CliService;

use super::templates::{run_migrations_cmd, validate_connection};
use crate::cloud::tenant::wait_for_postgres_healthy;

pub async fn handle_local_tenant_setup(
    cloud_user: &crate::cloud::sync::admin_user::CloudUser,
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
            let start_docker = Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt("PostgreSQL not running. Start Docker container?")
                .default(true)
                .interact()?;

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

        let run_migrations = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Run database migrations?")
            .default(true)
            .interact()?;

        if run_migrations {
            run_migrations_cmd(profile_path).await?;
        }
    }

    let result =
        crate::cloud::sync::admin_user::sync_admin_to_database(cloud_user, db_url, tenant_name)
            .await;

    match &result {
        crate::cloud::sync::admin_user::SyncResult::Created { email, .. } => {
            CliService::success(&format!("Created admin user: {}", email));
        },
        crate::cloud::sync::admin_user::SyncResult::Promoted { email, .. } => {
            CliService::success(&format!("Promoted user to admin: {}", email));
        },
        crate::cloud::sync::admin_user::SyncResult::AlreadyAdmin { email, .. } => {
            CliService::info(&format!("User '{}' is already admin", email));
        },
        crate::cloud::sync::admin_user::SyncResult::ConnectionFailed { error, .. } => {
            CliService::warning(&format!("Could not sync admin user: {}", error));
        },
        crate::cloud::sync::admin_user::SyncResult::Failed { error, .. } => {
            CliService::warning(&format!("Admin user sync failed: {}", error));
        },
    }

    Ok(())
}

pub fn get_cloud_user() -> Result<crate::cloud::sync::admin_user::CloudUser> {
    crate::cloud::sync::admin_user::CloudUser::from_credentials()?.ok_or_else(|| {
        anyhow::anyhow!("Cloud credentials required. Run 'systemprompt cloud login' first.")
    })
}

async fn start_postgres_container(compose_path: &Path) -> Result<bool> {
    CliService::info("Starting PostgreSQL container...");

    let compose_path_str = compose_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid compose path"))?;

    let status = Command::new("docker")
        .args(["compose", "-f", compose_path_str, "up", "-d"])
        .status()
        .map_err(|_| anyhow::anyhow!("Failed to execute docker compose. Is Docker running?"))?;

    if !status.success() {
        CliService::warning("Failed to start PostgreSQL container. Is Docker running?");
        return Ok(false);
    }

    let spinner = CliService::spinner("Waiting for PostgreSQL to be ready...");
    match wait_for_postgres_healthy(compose_path, 60).await {
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
