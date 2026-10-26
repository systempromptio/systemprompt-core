//! `admin config server cors` subcommands: manage the allowed-origins list.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::{Args, Subcommand};

use systemprompt_logging::CliService;

use super::super::profile_io::profile_path;
use super::super::types::{CorsListOutput, CorsModifyOutput};
use super::{load_profile, save_profile};
use crate::cli_settings::OutputFormat;
use crate::context::CommandContext;
use crate::shared::{CommandOutput, render_result};
use systemprompt_models::artifacts::ListItem;

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

pub(super) fn execute(command: &CorsCommands, ctx: &CommandContext) -> Result<()> {
    match command {
        CorsCommands::List => execute_list(ctx),
        CorsCommands::Add(args) => execute_add(args, ctx),
        CorsCommands::Remove(args) => execute_remove(args, ctx),
    }
}

fn execute_list(ctx: &CommandContext) -> Result<()> {
    let config = &ctx.cli;
    let profile = ctx.profile()?;

    let output = CorsListOutput {
        origins: profile.server.cors_allowed_origins.clone(),
        count: profile.server.cors_allowed_origins.len(),
    };

    let items: Vec<ListItem> = output
        .origins
        .into_iter()
        .map(|origin| ListItem::new(origin, String::new(), String::new()))
        .collect();
    render_result(
        &CommandOutput::list(items).with_title("CORS Allowed Origins"),
        config,
    );

    Ok(())
}

fn execute_add(args: &CorsAddArgs, ctx: &CommandContext) -> Result<()> {
    let config = &ctx.cli;
    let profile_path = profile_path(ctx)?;
    let mut profile = load_profile(profile_path)?;

    if profile.server.cors_allowed_origins.contains(&args.origin) {
        let output = CorsModifyOutput {
            action: "skipped".to_owned(),
            origin: args.origin.clone(),
            message: format!("Origin {} already exists", args.origin),
        };
        render_result(&CommandOutput::card_value("CORS Origin", &output), config);
        return Ok(());
    }

    profile.server.extra_cors_origins.push(args.origin.clone());
    save_profile(&profile, profile_path)?;

    let output = CorsModifyOutput {
        action: "added".to_owned(),
        origin: args.origin.clone(),
        message: format!("Added CORS origin: {}", args.origin),
    };
    render_result(
        &CommandOutput::card_value("CORS Origin Added", &output),
        config,
    );

    if config.output_format() == OutputFormat::Table {
        CliService::warning("Restart services for changes to take effect");
    }

    Ok(())
}

fn execute_remove(args: &CorsRemoveArgs, ctx: &CommandContext) -> Result<()> {
    let config = &ctx.cli;
    let profile_path = profile_path(ctx)?;
    let mut profile = load_profile(profile_path)?;

    let original_len = profile.server.extra_cors_origins.len();
    profile
        .server
        .extra_cors_origins
        .retain(|o| o != &args.origin);

    if profile.server.extra_cors_origins.len() == original_len {
        let output = CorsModifyOutput {
            action: "skipped".to_owned(),
            origin: args.origin.clone(),
            message: format!(
                "Origin {} is not an extra origin (the external_url origin is derived)",
                args.origin
            ),
        };
        render_result(&CommandOutput::card_value("CORS Origin", &output), config);
        return Ok(());
    }

    save_profile(&profile, profile_path)?;

    let output = CorsModifyOutput {
        action: "removed".to_owned(),
        origin: args.origin.clone(),
        message: format!("Removed CORS origin: {}", args.origin),
    };
    render_result(
        &CommandOutput::card_value("CORS Origin Removed", &output),
        config,
    );

    if config.output_format() == OutputFormat::Table {
        CliService::warning("Restart services for changes to take effect");
    }

    Ok(())
}
