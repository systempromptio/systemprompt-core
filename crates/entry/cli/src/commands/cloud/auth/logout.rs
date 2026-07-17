//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use systemprompt_cloud::{CloudApiClient, CloudCredentials, CloudPath, get_cloud_paths};
use systemprompt_logging::CliService;
use systemprompt_models::modules::ApiPaths;

use super::LogoutArgs;
use crate::cli_settings::CliConfig;
use crate::cloud::types::LogoutOutput;
use crate::interactive::Prompter;
use crate::shared::CommandOutput;

pub(super) async fn execute(
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

    let creds = CloudCredentials::load_from_path(&creds_path)?;
    let client = CloudApiClient::new(&creds.api_url, creds.api_token.as_str())?;

    std::fs::remove_file(&creds_path)?;

    let output = LogoutOutput {
        message: "Logged out of systemprompt.io Cloud".to_owned(),
        credentials_path: Some(creds_path.display().to_string()),
    };

    if !config.is_json_output() {
        CliService::key_value(
            "Removed credentials from",
            &creds_path.display().to_string(),
        );
        CliService::success("Logged out of systemprompt.io Cloud");
    }

    let activity_user_id = systemprompt_identifiers::UserId::new(creds.user_email.as_str());
    if let Err(e) = client
        .report_activity(ApiPaths::ACTIVITY_EVENT_LOGOUT, &activity_user_id)
        .await
    {
        tracing::debug!(error = %e, "Failed to report logout activity");
    }

    Ok(CommandOutput::card_value("Logout", &output))
}
