//! Delete a profile
//!
//! Removes profile directory including profile.yaml and secrets.json

use anyhow::{bail, Context, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use systemprompt_cloud::{ProfilePath, ProjectContext};
use systemprompt_core_logging::CliService;

pub fn execute(name: &str) -> Result<()> {
    CliService::section(&format!("Delete Profile: {}", name));

    let ctx = ProjectContext::discover();
    let profile_dir = ctx.profile_dir(name);

    if !profile_dir.exists() {
        bail!("Profile '{}' does not exist.", name);
    }

    let profile_yaml = ctx.profile_path(name, ProfilePath::Config);
    if !profile_yaml.exists() {
        bail!(
            "Directory '{}' exists but is not a profile (no profile.yaml).",
            name
        );
    }

    CliService::warning("The following will be deleted:");
    CliService::info(&format!("  {}", profile_dir.display()));

    for entry in std::fs::read_dir(&profile_dir)? {
        let entry = entry?;
        CliService::info(&format!("    - {}", entry.file_name().to_string_lossy()));
    }

    let confirmed = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Are you sure you want to delete this profile?")
        .default(false)
        .interact()?;

    if !confirmed {
        CliService::info("Cancelled.");
        return Ok(());
    }

    std::fs::remove_dir_all(&profile_dir)
        .with_context(|| format!("Failed to delete {}", profile_dir.display()))?;

    CliService::success(&format!("Deleted profile: {}", name));

    Ok(())
}
