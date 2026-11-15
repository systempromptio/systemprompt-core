//! `cloud auth logout` command clearing stored cloud state.
//!
//! Removes credentials, the synced tenant index, and tenant-scoped CLI
//! sessions; local sessions survive.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use systemprompt_cloud::{CloudPath, clear_cloud_state, get_cloud_paths};
use systemprompt_logging::CliService;

use super::LogoutArgs;
use crate::cli_settings::CliConfig;
use crate::cloud::types::LogoutOutput;
use crate::interactive::Prompter;
use crate::shared::CommandOutput;

pub(super) fn execute(
    args: LogoutArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<CommandOutput> {
    let cloud_paths = get_cloud_paths();
    let creds_path = cloud_paths.resolve(CloudPath::Credentials);

    if !creds_path.exists() {
        let output = LogoutOutput {
            message: "Already logged out (no credentials found)".to_owned(),
            credentials_path: None,
        };

        if !config.is_json_output() {
            CliService::success("Already logged out (no credentials found)");
        }

        return Ok(CommandOutput::card_value("Logout", &output));
    }

    if !args.yes {
        if !config.is_interactive() {
            return Err(anyhow::anyhow!(
                "--yes is required in non-interactive mode for logout"
            ));
        }

        let confirmed = prompter.confirm("Are you sure you want to log out?", false)?;

        if !confirmed {
            let output = LogoutOutput {
                message: "Cancelled".to_owned(),
                credentials_path: None,
            };

            if !config.is_json_output() {
                CliService::info("Cancelled.");
            }

            return Ok(CommandOutput::card_value("Logout", &output));
        }
    }

    let cleared = clear_cloud_state(&cloud_paths)?;

    let output = LogoutOutput {
        message: "Logged out of systemprompt.io Cloud".to_owned(),
        credentials_path: Some(creds_path.display().to_string()),
    };

    if !config.is_json_output() {
        if let Some(path) = &cleared.credentials_path {
            CliService::key_value("Removed credentials from", &path.display().to_string());
        }
        if let Some(path) = &cleared.tenants_path {
            CliService::key_value("Removed tenant index from", &path.display().to_string());
        }
        if cleared.tenant_sessions_removed > 0 {
            CliService::key_value(
                "Removed tenant sessions",
                &cleared.tenant_sessions_removed.to_string(),
            );
        }
        CliService::success("Logged out of systemprompt.io Cloud");
    }

    Ok(CommandOutput::card_value("Logout", &output))
}
