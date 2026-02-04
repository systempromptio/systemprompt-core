mod diff;
mod helpers;
mod import_export;
mod preset;
mod reset;
mod set;
mod show;
mod validate;

use anyhow::Result;
use clap::{Args, Subcommand};

use crate::shared::render_result;
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum RateLimitsCommands {
    #[command(about = "Show current rate limits configuration")]
    Show,

    #[command(about = "Show effective limits for a specific tier")]
    Tier(TierArgs),

    #[command(about = "Show rate limits documentation")]
    Docs,

    #[command(about = "Set a rate limit value")]
    Set(SetArgs),

    #[command(about = "Enable rate limiting")]
    Enable,

    #[command(about = "Disable rate limiting")]
    Disable,

    #[command(about = "Validate rate limit configuration")]
    Validate,

    #[command(about = "Compare effective limits across all tiers")]
    Compare,

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
pub struct TierArgs {
    #[arg(
        value_name = "TIER",
        help = "Tier name: admin, user, a2a, mcp, service, anon"
    )]
    pub tier: String,
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

    #[arg(
        long,
        help = "Tier to modify multiplier: admin, user, a2a, mcp, service, anon"
    )]
    pub tier: Option<String>,

    #[arg(long, help = "Multiplier value (requires --tier)")]
    pub multiplier: Option<f64>,

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

    #[arg(long, help = "Reset only this tier multiplier")]
    pub tier: Option<String>,
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

pub fn execute(command: RateLimitsCommands, config: &CliConfig) -> Result<()> {
    match command {
        RateLimitsCommands::Show => show::execute_show(config),
        RateLimitsCommands::Tier(args) => show::execute_tier(args, config),
        RateLimitsCommands::Docs => show::execute_docs(config),
        RateLimitsCommands::Set(args) => set::execute_set(&args, config),
        RateLimitsCommands::Enable => set::execute_enable(config),
        RateLimitsCommands::Disable => set::execute_disable(config),
        RateLimitsCommands::Validate => validate::execute_validate(config),
        RateLimitsCommands::Compare => validate::execute_compare(config),
        RateLimitsCommands::Reset(args) => reset::execute_reset(&args, config),
        RateLimitsCommands::Preset(cmd) => preset::execute_preset(cmd, config),
        RateLimitsCommands::Export(args) => {
            let result = import_export::execute_export(&args, config)?;
            render_result(&result);
            Ok(())
        },
        RateLimitsCommands::Import(args) => {
            let result = import_export::execute_import(&args, config)?;
            render_result(&result);
            Ok(())
        },
        RateLimitsCommands::Diff(args) => diff::execute_diff(&args, config),
    }
}
