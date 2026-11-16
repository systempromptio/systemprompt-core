//! `admin config services` command: show and edit profile-level services
//! overrides.
//!
//! [`ServicesCommands`] reports and updates the port offset applied to the
//! committed services manifests, letting a second installation on one host
//! move its MCP and agent ports without editing tracked config.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use anyhow::{Result, bail};
use crate::commands::admin::config::restart_note;
use clap::{Args, Subcommand};

use systemprompt_extension::cli::CommandOutput;
use super::profile_io::{load_profile, profile_path, save_profile};
use super::types::{ServicesConfigOutput, ServicesSetOutput};

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

pub fn execute(command: &ServicesCommands, host: &dyn KernelHost) -> Result<CommandOutput> {
    match command {
        ServicesCommands::Show => execute_show(host),
        ServicesCommands::Set(args) => execute_set(args, host),
    }
}

pub(super) fn execute_show(host: &dyn KernelHost) -> Result<CommandOutput> {
    let profile = host.profile()?;

    let output = ServicesConfigOutput {
        port_offset: profile.services.port_offset,
    };

    Ok(CommandOutput::card_value("Services Configuration", &output))
}

pub(super) fn execute_set(args: &SetArgs, host: &dyn KernelHost) -> Result<CommandOutput> {
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

    Ok(CommandOutput::card_value("Services Updated", &change).with_note(restart_note()))
}
