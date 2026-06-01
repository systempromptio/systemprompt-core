//! `admin config server cors` subcommands: manage the allowed-origins list.

use anyhow::Result;
use clap::{Args, Subcommand};
use systemprompt_config::ProfileBootstrap;
use systemprompt_logging::CliService;

use super::super::types::{CorsListOutput, CorsModifyOutput};
use super::{load_profile, save_profile};
use crate::CliConfig;
use crate::cli_settings::OutputFormat;
use crate::shared::{CommandResult, render_result};

#[derive(Debug, Subcommand)]
pub enum CorsCommands {
    #[command(about = "List CORS allowed origins")]
    List,

    #[command(about = "Add a CORS origin")]
    Add(CorsAddArgs),

    #[command(about = "Remove a CORS origin")]
    Remove(CorsRemoveArgs),
}

#[derive(Debug, Clone, Args)]
pub struct CorsAddArgs {
    #[arg(help = "Origin URL to add (e.g., https://example.com)")]
    pub origin: String,
}

#[derive(Debug, Clone, Args)]
pub struct CorsRemoveArgs {
    #[arg(help = "Origin URL to remove")]
    pub origin: String,
}

pub(super) fn execute(command: &CorsCommands, config: &CliConfig) -> Result<()> {
    match command {
        CorsCommands::List => execute_list(),
        CorsCommands::Add(args) => execute_add(args, config),
        CorsCommands::Remove(args) => execute_remove(args, config),
    }
}

fn execute_list() -> Result<()> {
    let profile = ProfileBootstrap::get()?;

    let output = CorsListOutput {
        origins: profile.server.cors_allowed_origins.clone(),
        count: profile.server.cors_allowed_origins.len(),
    };

    render_result(&CommandResult::list(output).with_title("CORS Allowed Origins"));

    Ok(())
}

fn execute_add(args: &CorsAddArgs, config: &CliConfig) -> Result<()> {
    let profile_path = ProfileBootstrap::get_path()?;
    let mut profile = load_profile(profile_path)?;

    if profile.server.cors_allowed_origins.contains(&args.origin) {
        let output = CorsModifyOutput {
            action: "skipped".to_owned(),
            origin: args.origin.clone(),
            message: format!("Origin {} already exists", args.origin),
        };
        render_result(&CommandResult::text(output).with_title("CORS Origin"));
        return Ok(());
    }

    profile
        .server
        .cors_allowed_origins
        .push(args.origin.clone());
    save_profile(&profile, profile_path)?;

    let output = CorsModifyOutput {
        action: "added".to_owned(),
        origin: args.origin.clone(),
        message: format!("Added CORS origin: {}", args.origin),
    };
    render_result(&CommandResult::text(output).with_title("CORS Origin Added"));

    if config.output_format() == OutputFormat::Table {
        CliService::warning("Restart services for changes to take effect");
    }

    Ok(())
}

fn execute_remove(args: &CorsRemoveArgs, config: &CliConfig) -> Result<()> {
    let profile_path = ProfileBootstrap::get_path()?;
    let mut profile = load_profile(profile_path)?;

    let original_len = profile.server.cors_allowed_origins.len();
    profile
        .server
        .cors_allowed_origins
        .retain(|o| o != &args.origin);

    if profile.server.cors_allowed_origins.len() == original_len {
        let output = CorsModifyOutput {
            action: "skipped".to_owned(),
            origin: args.origin.clone(),
            message: format!("Origin {} not found", args.origin),
        };
        render_result(&CommandResult::text(output).with_title("CORS Origin"));
        return Ok(());
    }

    save_profile(&profile, profile_path)?;

    let output = CorsModifyOutput {
        action: "removed".to_owned(),
        origin: args.origin.clone(),
        message: format!("Removed CORS origin: {}", args.origin),
    };
    render_result(&CommandResult::text(output).with_title("CORS Origin Removed"));

    if config.output_format() == OutputFormat::Table {
        CliService::warning("Restart services for changes to take effect");
    }

    Ok(())
}
