use anyhow::Result;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use std::path::PathBuf;
use systemprompt_cloud::{ProfilePath, ProjectContext};
use systemprompt_logging::CliService;

use super::{show, ShowFilter};
use crate::cli_settings::CliConfig;

pub fn execute(config: &CliConfig) -> Result<()> {
    let ctx = ProjectContext::discover();
    let profiles_dir = ctx.profiles_dir();

    if !profiles_dir.exists() {
        CliService::section("Profiles");
        CliService::warning("No profiles found.");
        CliService::info("Run 'systemprompt cloud profile create <name>' to create a profile.");
        return Ok(());
    }

    let current_profile = std::env::var("SYSTEMPROMPT_PROFILE").ok();
    let current_profile_name = current_profile.as_ref().and_then(|p| {
        let path = PathBuf::from(p);
        path.parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
            .map(String::from)
    });

    let mut profiles = Vec::new();

    for entry in std::fs::read_dir(&profiles_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let profile_yaml = ProfilePath::Config.resolve(&path);
            if profile_yaml.exists() {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");

                let has_secrets = ProfilePath::Secrets.resolve(&path).exists();

                profiles.push((name.to_string(), has_secrets, path.clone()));
            }
        }
    }

    if profiles.is_empty() {
        CliService::section("Profiles");
        CliService::warning("No profiles found.");
        CliService::info("Run 'systemprompt cloud profile create <name>' to create a profile.");
        return Ok(());
    }

    profiles.sort_by(|a, b| a.0.cmp(&b.0));

    if !config.is_interactive() {
        CliService::section("Profiles");
        for (name, has_secrets, _) in &profiles {
            let is_current = current_profile_name.as_ref().is_some_and(|c| c == name);
            let current_marker = if is_current { " (active)" } else { "" };
            let secrets_marker = if *has_secrets { "✓" } else { "✗" };
            CliService::info(&format!(
                "{}{} [secrets: {}]",
                name, current_marker, secrets_marker
            ));
        }
        return Ok(());
    }

    let options: Vec<String> = profiles
        .iter()
        .map(|(name, has_secrets, _)| {
            let is_current = current_profile_name.as_ref().is_some_and(|c| c == name);
            let current_marker = if is_current { " (active)" } else { "" };
            let secrets_marker = if *has_secrets { "✓" } else { "✗" };
            format!("{}{} [secrets: {}]", name, current_marker, secrets_marker)
        })
        .chain(std::iter::once("Back".to_string()))
        .collect();

    loop {
        CliService::section("Profiles");

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select profile")
            .items(&options)
            .default(0)
            .interact()?;

        if selection == profiles.len() {
            break;
        }

        let (profile_name, _, _) = &profiles[selection];
        show::execute(Some(profile_name), ShowFilter::All, false, false, config)?;
    }

    Ok(())
}
