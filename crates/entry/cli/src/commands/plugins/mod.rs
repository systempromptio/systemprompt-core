//! `plugins` command group: discover and inspect compiled extensions.
//!
//! Covers listing and showing extensions, running CLI extension commands,
//! validating dependencies and configuration, dumping config, summarising
//! [`PluginsCommands::Capabilities`], listing link-time registration
//! [`seams`], and the [`mcp`] server-management
//! subtree. [`types`] holds the shared serialisable output shapes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod types;

pub mod capabilities;
pub mod config;
pub mod list;
mod run;
pub mod seams;
pub mod show;
pub mod validate;

pub mod mcp;

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use systemprompt_extension::ExtensionRegistry;

use crate::context::CommandContext;
use crate::descriptor::{CommandDescriptor, DataImpact, DescribeCommand};
use crate::shared::render_result;

fn owned_jobs(owner: &str) -> impl Iterator<Item = &'static dyn systemprompt_traits::Job> + '_ {
    systemprompt_traits::registered_jobs()
        .filter(move |registration| registration.owner == owner)
        .map(|registration| registration.job)
}

fn discover_registry() -> ExtensionRegistry {
    ExtensionRegistry::discover().unwrap_or_else(|e| {
        tracing::error!(error = %e, "extension dependency cycle; using empty registry");
        ExtensionRegistry::new()
    })
}

#[derive(Debug, Subcommand)]
pub enum PluginsCommands {
    #[command(about = "List all discovered extensions")]
    List(list::ListArgs),

    #[command(about = "Show detailed extension information")]
    Show(show::ShowArgs),

    #[command(about = "Run a CLI extension command", trailing_var_arg = true)]
    Run(run::RunArgs),

    #[command(about = "Validate extension dependencies and configurations")]
    Validate(validate::ValidateArgs),

    #[command(about = "Show extension configuration")]
    Config(config::ConfigArgs),

    #[command(about = "List capabilities across all extensions")]
    Capabilities(capabilities::CapabilitiesArgs),

    #[command(about = "List every link-time registration seam and check the profile against it")]
    Seams(seams::SeamsArgs),

    #[command(subcommand, about = "MCP server management")]
    Mcp(mcp::McpCommands),
}

impl DescribeCommand for PluginsCommands {
    fn descriptor(&self) -> CommandDescriptor {
        match self {
            Self::Run(_) => CommandDescriptor::PROFILE_AND_SECRETS,
            Self::Mcp(_) => CommandDescriptor::FULL,
            _ => CommandDescriptor::PROFILE_ONLY.with_remote_eligible(),
        }
    }
}

pub async fn execute(cmd: PluginsCommands, ctx: &CommandContext) -> Result<()> {
    match cmd {
        PluginsCommands::List(args) => {
            render_result(&list::execute(&args, &ctx.cli), &ctx.cli);
            Ok(())
        },
        PluginsCommands::Show(args) => {
            let result = show::execute(&args, &ctx.cli).context("Failed to show extension")?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        PluginsCommands::Run(args) => run::execute(args, ctx).await,
        PluginsCommands::Validate(args) => {
            let (output, valid) = validate::execute(&args, &ctx.cli);
            render_result(&output, &ctx.cli);
            if !valid {
                bail!("Extension validation failed");
            }
            Ok(())
        },
        PluginsCommands::Config(args) => {
            let result =
                config::execute(&args, &ctx.cli).context("Failed to get extension config")?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        PluginsCommands::Capabilities(args) => {
            capabilities::execute(args, &ctx.cli);
            Ok(())
        },
        PluginsCommands::Seams(args) => {
            let (output, all_linked) = seams::execute(&args, ctx.profile().ok());
            render_result(&output, &ctx.cli);
            if !all_linked {
                bail!("The profile names registrations this binary does not link");
            }
            Ok(())
        },
        PluginsCommands::Mcp(cmd) => Box::pin(mcp::execute(cmd, ctx)).await,
    }
}

impl PluginsCommands {
    pub const fn data_impact(&self) -> DataImpact {
        match self {
            Self::List(_)
            | Self::Show(_)
            | Self::Run(_)
            | Self::Validate(_)
            | Self::Config(_)
            | Self::Capabilities(_)
            | Self::Seams(_)
            | Self::Mcp(
                mcp::McpCommands::List(_)
                | mcp::McpCommands::Status(_)
                | mcp::McpCommands::Validate(_)
                | mcp::McpCommands::Logs(_)
                | mcp::McpCommands::ListPackages(_)
                | mcp::McpCommands::Tools(_)
                | mcp::McpCommands::Call(_),
            ) => DataImpact::Preserving,
        }
    }
}
