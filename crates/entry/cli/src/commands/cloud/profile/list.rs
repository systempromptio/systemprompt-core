//! `cloud profile list`: enumerate the profiles under the project.
//!
//! Scans the profiles directory, marks the active profile and secret
//! presence, and either renders a table or drives an interactive
//! profile-picker that drills into [`show`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use std::path::{Path, PathBuf};
use systemprompt_cloud::{ProfilePath, ProjectContext};
use systemprompt_logging::CliService;

use super::{ShowFilter, show};
use crate::cli_settings::CliConfig;
use crate::cloud::types::{ProfileListOutput, ProfileSummary};
use crate::context::CommandContext;
use crate::interactive::Prompter;
use crate::shared::CommandOutput;

pub(super) fn execute(ctx: &CommandContext) -> Result<CommandOutput> {
    let config = &ctx.cli;
    let project = ProjectContext::discover();
    let profiles_dir = project.profiles_dir();

    if !profiles_dir.exists() {
        return Ok(render_no_profiles(config));
    }

    let current_profile_name = active_profile_name(ctx.env.profile.as_deref());

    let mut profiles = scan_profiles(&profiles_dir)?;

    if profiles.is_empty() {
        return Ok(render_no_profiles(config));
    }

    profiles.sort_by(|a, b| a.0.cmp(&b.0));

    let summaries: Vec<ProfileSummary> = profiles
        .iter()
        .map(|(name, has_secrets, _)| ProfileSummary {
            name: name.clone(),
            has_secrets: *has_secrets,
            is_active: current_profile_name.as_ref().is_some_and(|c| c == name),
        })
        .collect();

    let output = ProfileListOutput {
        total: summaries.len(),
        profiles: summaries,
        active_profile: current_profile_name.clone(),
    };

    if !config.is_json_output() {
        if config.is_interactive() {
            run_profile_picker(
                ctx.prompter(),
                &profiles,
                current_profile_name.as_ref(),
                ctx,
            )?;
        } else {
            render_profile_lines(&profiles, current_profile_name.as_ref());
        }
    }

    Ok(
        CommandOutput::table_of(vec!["name", "has_secrets", "is_active"], &output.profiles)
            .with_title("Profiles"),
    )
}

fn active_profile_name(profile_env: Option<&str>) -> Option<String> {
    profile_env.and_then(|p| {
        let path = PathBuf::from(p);
        path.parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
            .map(String::from)
    })
}

fn scan_profiles(profiles_dir: &Path) -> Result<Vec<(String, bool, PathBuf)>> {
    let mut profiles = Vec::new();

    for entry in std::fs::read_dir(profiles_dir)? {
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

                profiles.push((name.to_owned(), has_secrets, path.clone()));
            }
        }
    }

    Ok(profiles)
}

fn render_no_profiles(config: &CliConfig) -> CommandOutput {
    if !config.is_json_output() {
        CliService::section("Profiles");
        CliService::warning("No profiles found.");
        CliService::info("Run 'systemprompt cloud profile create <name>' to create a profile.");
    }

    let profiles: Vec<ProfileSummary> = Vec::new();
    CommandOutput::table_of(vec!["name", "has_secrets", "is_active"], &profiles)
        .with_title("Profiles")
}

fn profile_label(name: &str, has_secrets: bool, current: Option<&String>) -> String {
    let is_current = current.is_some_and(|c| c == name);
    let current_marker = if is_current { " (active)" } else { "" };
    let secrets_marker = if has_secrets { "✓" } else { "✗" };
    format!("{}{} [secrets: {}]", name, current_marker, secrets_marker)
}

fn run_profile_picker(
    prompter: &dyn Prompter,
    profiles: &[(String, bool, PathBuf)],
    current: Option<&String>,
    ctx: &CommandContext,
) -> Result<()> {
    let options: Vec<String> = profiles
        .iter()
        .map(|(name, has_secrets, _)| profile_label(name, *has_secrets, current))
        .chain(std::iter::once("Back".to_owned()))
        .collect();

    loop {
        CliService::section("Profiles");

        let selection = prompter.select("Select profile", &options)?;

        if selection == profiles.len() {
            break;
        }

        let (profile_name, _, _) = &profiles[selection];
        show::execute(Some(profile_name), ShowFilter::All, false, false, ctx)?;
    }

    Ok(())
}

fn render_profile_lines(profiles: &[(String, bool, PathBuf)], current: Option<&String>) {
    CliService::section("Profiles");
    for (name, has_secrets, _) in profiles {
        CliService::info(&profile_label(name, *has_secrets, current));
    }
}
