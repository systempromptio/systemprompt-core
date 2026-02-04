use anyhow::Result;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use systemprompt_cloud::{get_cloud_paths, CloudApiClient, CloudCredentials, CloudPath};
use systemprompt_logging::CliService;
use systemprompt_models::modules::ApiPaths;

use super::LogoutArgs;
use crate::cli_settings::CliConfig;
use crate::cloud::types::LogoutOutput;
use crate::shared::CommandResult;

pub async fn execute(args: LogoutArgs, config: &CliConfig) -> Result<CommandResult<LogoutOutput>> {
    let cloud_paths = get_cloud_paths()?;
    let creds_path = cloud_paths.resolve(CloudPath::Credentials);

    if !creds_path.exists() {
        let output = LogoutOutput {
            message: "Already logged out (no credentials found)".to_string(),
            credentials_path: None,
        };

        if !config.is_json_output() {
            CliService::success("Already logged out (no credentials found)");
        }

        return Ok(CommandResult::text(output).with_title("Logout"));
    }

    if !args.yes {
        if !config.is_interactive() {
            return Err(anyhow::anyhow!(
                "--yes is required in non-interactive mode for logout"
            ));
        }

        let confirmed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Are you sure you want to log out?")
            .default(false)
            .interact()?;

        if !confirmed {
            let output = LogoutOutput {
                message: "Cancelled".to_string(),
                credentials_path: None,
            };

            if !config.is_json_output() {
                CliService::info("Cancelled.");
            }

            return Ok(CommandResult::text(output).with_title("Logout"));
        }
    }

    let creds = CloudCredentials::load_from_path(&creds_path)?;
    let client = CloudApiClient::new(&creds.api_url, &creds.api_token);

    std::fs::remove_file(&creds_path)?;

    let output = LogoutOutput {
        message: "Logged out of systemprompt.io Cloud".to_string(),
        credentials_path: Some(creds_path.display().to_string()),
    };

    if !config.is_json_output() {
        CliService::key_value(
            "Removed credentials from",
            &creds_path.display().to_string(),
        );
        CliService::success("Logged out of systemprompt.io Cloud");
    }

    if let Err(e) = client
        .report_activity(ApiPaths::ACTIVITY_EVENT_LOGOUT, &creds.user_email)
        .await
    {
        tracing::debug!(error = %e, "Failed to report logout activity");
    }

    Ok(CommandResult::text(output).with_title("Logout"))
}
