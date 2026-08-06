//! `admin config services` command: show and edit profile-level services
//! overrides.
//!
//! [`ServicesCommands`] reports and updates the port offset applied to the
//! committed services manifests, letting a second installation on one host
//! move its MCP and agent ports without editing tracked config.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use std::fs;
use systemprompt_config::ProfileBootstrap;
use systemprompt_logging::CliService;
use systemprompt_models::Profile;

use super::runtime::save_profile;
use super::types::{ServicesConfigOutput, ServicesSetOutput};
use crate::CliConfig;
use crate::cli_settings::OutputFormat;
use crate::shared::{CommandOutput, render_result};

#[derive(Debug, Clone, Copy, Subcommand)]
pub enum ServicesCommands {
    #[command(about = "Show services configuration", alias = "list")]
    Show,

    #[command(about = "Set services configuration value")]
    Set(SetArgs),
}

#[derive(Debug, Clone, Copy, Args)]
pub struct SetArgs {
    #[arg(
        long,
        help = "Shift every locally-bound MCP and agent port by this amount"
    )]
    pub port_offset: Option<u16>,
}

pub fn execute(command: &ServicesCommands, config: &CliConfig) -> Result<()> {
    match command {
        ServicesCommands::Show => execute_show(config),
        ServicesCommands::Set(args) => execute_set(args, config),
    }
}

pub(super) fn execute_show(config: &CliConfig) -> Result<()> {
    let profile = ProfileBootstrap::get()?;

    let output = ServicesConfigOutput {
        port_offset: profile.services.port_offset,
    };

    render_result(
        &CommandOutput::card_value("Services Configuration", &output),
        config,
    );

    Ok(())
}

pub(super) fn execute_set(args: &SetArgs, config: &CliConfig) -> Result<()> {
    let Some(port_offset) = args.port_offset else {
        bail!("Must specify at least one option: --port-offset");
    };

    let profile_path = ProfileBootstrap::get_path()?;
    let mut profile = load_profile(profile_path)?;

    let old = profile.services.port_offset;
    profile.services.port_offset = port_offset;

    save_profile(&profile, profile_path)?;

    let change = ServicesSetOutput {
        field: "port_offset".to_owned(),
        old_value: old.to_string(),
        new_value: port_offset.to_string(),
        message: format!("Updated port_offset to {port_offset}"),
    };

    render_result(
        &CommandOutput::card_value("Services Updated", &change),
        config,
    );

    if config.output_format() == OutputFormat::Table {
        CliService::warning("Restart services for changes to take effect");
    }

    Ok(())
}

fn load_profile(path: &str) -> Result<Profile> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read profile: {path}"))?;
    let profile: Profile = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse profile: {path}"))?;
    Ok(profile)
}
