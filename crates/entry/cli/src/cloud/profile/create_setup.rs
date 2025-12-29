use anyhow::Result;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use std::path::Path;
use systemprompt_core_logging::CliService;

use super::templates::{run_migrations_cmd, validate_connection};

pub async fn handle_local_tenant_setup(
    cloud_user: &crate::cloud::sync::admin_user::CloudUser,
    db_url: &str,
    name: &str,
    profile_path: &Path,
) -> Result<()> {
    let spinner = CliService::spinner("Validating PostgreSQL connection...");
    let connection_valid = validate_connection(db_url).await;
    spinner.finish_and_clear();

    if connection_valid {
        CliService::success("PostgreSQL connection verified");

        let run_migrations = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Run database migrations?")
            .default(true)
            .interact()?;

        if run_migrations {
            run_migrations_cmd(profile_path).await?;
        }
    } else {
        CliService::warning("Could not connect to PostgreSQL.");
        CliService::info("Ensure PostgreSQL is running before starting services.");
    }

    let result =
        crate::cloud::sync::admin_user::sync_admin_to_database(cloud_user, db_url, name).await;

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
