use anyhow::{bail, Context, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use std::path::PathBuf;
use systemprompt_cloud::{ProfilePath, ProjectContext};
use systemprompt_core_logging::CliService;
use systemprompt_loader::ProfileLoader;

use super::edit_secrets::edit_api_keys;
use super::edit_settings::{edit_runtime_settings, edit_security_settings, edit_server_settings};
use super::templates::save_profile;
use crate::cli_settings::CliConfig;

pub async fn execute(name: Option<&str>, config: &CliConfig) -> Result<()> {
    if !config.is_interactive() {
        return Err(anyhow::anyhow!(
            "Profile edit requires interactive mode.\n\
             Use specific commands to modify profile settings in non-interactive mode."
        ));
    }

    let profile_path = resolve_profile_path(name)?;
    let profile_dir = profile_path
        .parent()
        .context("Invalid profile path")?
        .to_path_buf();

    CliService::section(&format!("Edit Profile: {}", profile_path.display()));

    let mut profile = ProfileLoader::load_from_path(&profile_path)?;

    let edit_options = vec![
        "Server settings (host, port, URLs)",
        "Security settings (JWT)",
        "Runtime settings (environment, log level)",
        "API keys (secrets.json)",
        "Done - save and exit",
    ];

    loop {
        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("What would you like to edit?")
            .items(&edit_options)
            .default(0)
            .interact()?;

        match selection {
            0 => edit_server_settings(&mut profile)?,
            1 => edit_security_settings(&mut profile)?,
            2 => edit_runtime_settings(&mut profile)?,
            3 => edit_api_keys(&profile_dir).await?,
            4 => break,
            _ => unreachable!(),
        }
    }

    save_profile(&profile, &profile_path)?;
    CliService::success(&format!("Profile saved: {}", profile_path.display()));

    Ok(())
}

fn resolve_profile_path(name: Option<&str>) -> Result<PathBuf> {
    if let Some(profile_name) = name {
        let ctx = ProjectContext::discover();
        let profile_path = ctx.profile_path(profile_name, ProfilePath::Config);

        if !profile_path.exists() {
            bail!(
                "Profile '{}' not found at {}",
                profile_name,
                profile_path.display()
            );
        }

        return Ok(profile_path);
    }

    if let Ok(path) = std::env::var("SYSTEMPROMPT_PROFILE") {
        let profile_path = PathBuf::from(&path);
        if profile_path.exists() {
            return Ok(profile_path);
        }
        bail!("Profile from SYSTEMPROMPT_PROFILE not found: {}", path);
    }

    select_profile_interactively()
}

fn select_profile_interactively() -> Result<PathBuf> {
    let ctx = ProjectContext::discover();
    let profiles_dir = ctx.profiles_dir();

    if !profiles_dir.exists() {
        bail!("No profiles directory found. Run 'systemprompt cloud profile create' first.");
    }

    let profiles: Vec<String> = std::fs::read_dir(&profiles_dir)?
        .filter_map(|e| {
            e.map_err(|err| {
                tracing::debug!(error = %err, "Failed to read profile directory entry");
                err
            })
            .ok()
        })
        .filter(|e| e.path().is_dir() && ProfilePath::Config.resolve(&e.path()).exists())
        .filter_map(|e| e.file_name().to_str().map(String::from))
        .collect();

    if profiles.is_empty() {
        bail!("No profiles found. Run 'systemprompt cloud profile create' first.");
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select profile to edit")
        .items(&profiles)
        .default(0)
        .interact()?;

    let selected = &profiles[selection];
    Ok(ctx.profile_path(selected, ProfilePath::Config))
}
