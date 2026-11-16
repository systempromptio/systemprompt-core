//! `admin config services` command: show and edit profile-level services
//! overrides.
//!
//! [`ServicesCommands`] reports and updates the port offset applied to the
//! committed services manifests, letting a second installation on one host
//! move its MCP and agent ports without editing tracked config.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use systemprompt_logging::CliService;

use crate::cli_settings::OutputFormat;
use crate::shared::CommandOutput;
use super::profile_io::{load_profile, profile_path, save_profile};
use super::types::{ServicesConfigOutput, ServicesSetOutput};
use systemprompt_extension::cli::CommandHost;

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

pub fn execute(command: &ServicesCommands, host: &dyn CommandHost) -> Result<()> {
    match command {
        ServicesCommands::Show => execute_show(host),
        ServicesCommands::Set(args) => execute_set(args, host),
    }
}

pub(super) fn execute_show(host: &dyn CommandHost) -> Result<()> {
    let config = host.output();
    let profile = host.profile()?;

    let output = ServicesConfigOutput {
        port_offset: profile.services.port_offset,
    };

    host.render(&CommandOutput::card_value("Services Configuration", &output));

    Ok(())
}

pub(super) fn execute_set(args: &SetArgs, host: &dyn CommandHost) -> Result<()> {
    let config = host.output();
    let Some(port_offset) = args.port_offset else {
        bail!("Must specify at least one option: --port-offset");
    };

    let profile_path = profile_path(host)?;
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

    host.render(&CommandOutput::card_value("Services Updated", &change));

    if config.output_format() == OutputFormat::Table {
        CliService::warning("Restart services for changes to take effect");
    }

    Ok(())
}
