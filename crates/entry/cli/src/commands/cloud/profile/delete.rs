use anyhow::{bail, Context, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use systemprompt_cloud::{ProfilePath, ProjectContext};
use systemprompt_logging::CliService;

use super::DeleteArgs;
use crate::cli_settings::CliConfig;
use crate::shared::{CommandResult, SuccessOutput};

pub fn execute(args: &DeleteArgs, config: &CliConfig) -> Result<CommandResult<SuccessOutput>> {
    if !config.is_json_output() {
        CliService::section(&format!("Delete Profile: {}", args.name));
    }

    let ctx = ProjectContext::discover();
    let profile_dir = ctx.profile_dir(&args.name);

    if !profile_dir.exists() {
        bail!("Profile '{}' does not exist.", args.name);
    }

    let profile_yaml = ctx.profile_path(&args.name, ProfilePath::Config);
    if !profile_yaml.exists() {
        bail!(
            "Directory '{}' exists but is not a profile (no profile.yaml).",
            args.name
        );
    }

    if !config.is_json_output() {
        CliService::warning("The following will be deleted:");
        CliService::info(&format!("  {}", profile_dir.display()));

        for entry in std::fs::read_dir(&profile_dir)? {
            let entry = entry?;
            CliService::info(&format!("    - {}", entry.file_name().to_string_lossy()));
        }
    }

    if !args.yes {
        if !config.is_interactive() {
            return Err(anyhow::anyhow!(
                "--yes is required in non-interactive mode for profile delete"
            ));
        }

        let confirmed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Are you sure you want to delete this profile?")
            .default(false)
            .interact()?;

        if !confirmed {
            if !config.is_json_output() {
                CliService::info("Cancelled.");
            }
            let output = SuccessOutput::new("Cancelled");
            return Ok(CommandResult::text(output).with_title("Delete Profile"));
        }
    }

    std::fs::remove_dir_all(&profile_dir)
        .with_context(|| format!("Failed to delete {}", profile_dir.display()))?;

    let output = SuccessOutput::new(format!("Deleted profile: {}", args.name));

    if !config.is_json_output() {
        CliService::success(&format!("Deleted profile: {}", args.name));
    }

    Ok(CommandResult::text(output).with_title("Delete Profile"))
}
