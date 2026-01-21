use anyhow::Result;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use systemprompt_cloud::{get_cloud_paths, CloudPath};
use systemprompt_logging::CliService;

use super::LogoutArgs;
use crate::cli_settings::CliConfig;

pub fn execute(args: LogoutArgs, config: &CliConfig) -> Result<()> {
    let cloud_paths = get_cloud_paths()?;
    let creds_path = cloud_paths.resolve(CloudPath::Credentials);

    if !creds_path.exists() {
        CliService::success("Already logged out (no credentials found)");
        return Ok(());
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
            CliService::info("Cancelled.");
            return Ok(());
        }
    }

    std::fs::remove_file(&creds_path)?;
    CliService::key_value(
        "Removed credentials from",
        &creds_path.display().to_string(),
    );
    CliService::success("Logged out of SystemPrompt Cloud");

    Ok(())
}
