//! `admin config rate-limits` command tree: inspect, edit, and compare rate
//! limits.
//!
//! [`RateLimitsCommands`] covers showing the configured limits, setting
//! endpoint rates, validating the configuration, applying presets, and
//! importing/exporting/diffing against files or defaults. The editing paths
//! persist changes to the active profile YAML.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod diff;
mod import_export;
pub mod preset;
mod profile_edit;
mod reset;
mod set;
mod show;
mod validate;

use anyhow::Result;
use clap::{Args, Subcommand};

use crate::context::CommandContext;
use crate::interactive::Prompter;
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum RateLimitsCommands {
    #[command(about = "Show current rate limits configuration", alias = "list")]
    Show,

    #[command(about = "Set a rate limit value")]
    Set(SetArgs),

    #[command(about = "Enable rate limiting")]
    Enable,

    #[command(about = "Disable rate limiting")]
    Disable,

    #[command(about = "Validate rate limit configuration")]
    Validate,

    #[command(about = "Reset rate limits to defaults")]
    Reset(ResetArgs),

    #[command(subcommand, about = "Manage rate limit presets")]
    Preset(PresetCommands),

    #[command(about = "Export rate limits to file")]
    Export(ExportArgs),

    #[command(about = "Import rate limits from file")]
    Import(ImportArgs),

    #[command(about = "Compare rate limits with defaults or file")]
    Diff(DiffArgs),
}

#[derive(Debug, Clone, Args)]
pub struct SetArgs {
    #[arg(
        long,
        help = "Endpoint to modify: oauth_public, oauth_auth, contexts, tasks, artifacts, \
                agent_registry, agents, mcp_registry, mcp, stream, content"
    )]
    pub endpoint: Option<String>,

    #[arg(long, help = "Rate per second (requires --endpoint)")]
    pub rate: Option<u64>,

    #[arg(long, help = "Burst multiplier value")]
    pub burst: Option<u64>,
}

#[derive(Debug, Clone, Args)]
pub struct ResetArgs {
    #[arg(short = 'y', long, help = "Skip confirmation")]
    pub yes: bool,

    #[arg(long, help = "Preview changes without applying")]
    pub dry_run: bool,

    #[arg(long, help = "Reset only this endpoint")]
    pub endpoint: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum PresetCommands {
    #[command(about = "List available presets")]
    List,

    #[command(about = "Show preset configuration")]
    Show(PresetShowArgs),

    #[command(about = "Apply a preset")]
    Apply(PresetApplyArgs),
}

#[derive(Debug, Clone, Args)]
pub struct PresetShowArgs {
    #[arg(help = "Preset name: development, production, high-traffic")]
    pub name: String,
}

#[derive(Debug, Clone, Args)]
pub struct PresetApplyArgs {
    #[arg(help = "Preset name: development, production, high-traffic")]
    pub name: String,

    #[arg(short = 'y', long, help = "Skip confirmation")]
    pub yes: bool,
}

#[derive(Debug, Clone, Args)]
pub struct ExportArgs {
    #[arg(long, short = 'o', help = "Output file path")]
    pub output: String,

    #[arg(long, default_value = "yaml", help = "Format: yaml, json")]
    pub format: String,
}

#[derive(Debug, Clone, Args)]
pub struct ImportArgs {
    #[arg(long, short = 'f', help = "Input file path")]
    pub file: String,

    #[arg(short = 'y', long, help = "Skip confirmation")]
    pub yes: bool,
}

#[derive(Debug, Clone, Args)]
pub struct DiffArgs {
    #[arg(long, help = "Compare with defaults")]
    pub defaults: bool,

    #[arg(long, short = 'f', help = "Compare with file")]
    pub file: Option<String>,
}

pub fn execute(
    command: RateLimitsCommands,
    prompter: &dyn Prompter,
    ctx: &CommandContext,
) -> Result<()> {
    let config = &ctx.cli;
    match command {
        RateLimitsCommands::Show => show::execute_show(ctx),
        RateLimitsCommands::Set(args) => set::execute_set(&args, ctx),
        RateLimitsCommands::Enable => set::execute_enable(ctx),
        RateLimitsCommands::Disable => set::execute_disable(ctx),
        RateLimitsCommands::Validate => validate::execute_validate(ctx),
        RateLimitsCommands::Reset(args) => reset::execute_reset(&args, prompter, ctx),
        RateLimitsCommands::Preset(cmd) => preset::execute_preset(cmd, prompter, ctx),
        RateLimitsCommands::Export(args) => {
            let result = import_export::execute_export(&args, ctx)?;
            render_result(&result, config);
            Ok(())
        },
        RateLimitsCommands::Import(args) => {
            let result = import_export::execute_import(&args, prompter, ctx)?;
            render_result(&result, config);
            Ok(())
        },
        RateLimitsCommands::Diff(args) => diff::execute_diff(&args, ctx),
    }
}
