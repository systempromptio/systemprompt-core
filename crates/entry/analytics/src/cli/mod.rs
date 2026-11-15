//! The `analytics` command group and the `core content popular` command.
//!
//! [`AnalyticsCommands`] is a top-level group registered with the CLI kernel
//! in the `analytics` `--help` slot. It routes to the per-domain subcommand
//! trees (overview, conversations, agents, tools, requests, sessions,
//! content, traffic, costs); each variant dispatches to its module's
//! `execute` against the invocation's [`CommandContext`]. Every analytics
//! command is read-only, preserves the profile's data, runs against
//! `--database-url` directly and writes a `--export` file locally. The
//! [`shared`] module holds the time-range and output formatting helpers used
//! across all domains.
//!
//! [`content_popular::PopularArgs`] is registered as the `popular` leaf under
//! the kernel's `core content` group.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod agents;
pub mod content;
pub mod content_popular;
pub mod conversations;
pub mod costs;
pub mod overview;
pub mod requests;
pub mod sessions;
pub mod shared;
pub mod tools;
pub mod traffic;

use anyhow::Result;
use clap::Subcommand;
use systemprompt_cli::kernel::{
    CliCommandGroup, CommandContext, CommandDescriptor, DataImpact, DbUrlRouting, DescribeCommand,
    render_result,
};

use crate::cli::shared::with_resolved_export;

#[derive(Debug, Subcommand)]
pub enum AnalyticsCommands {
    #[command(about = "Dashboard overview of all analytics")]
    Overview(overview::OverviewArgs),

    #[command(subcommand, about = "Conversation analytics")]
    Conversations(conversations::ConversationsCommands),

    #[command(subcommand, about = "Agent performance analytics")]
    Agents(agents::AgentsCommands),

    #[command(subcommand, about = "Tool usage analytics")]
    Tools(tools::ToolsCommands),

    #[command(about = "AI request analytics; defaults to the stats aggregate")]
    Requests(requests::RequestsArgs),

    #[command(subcommand, about = "Session analytics")]
    Sessions(sessions::SessionsCommands),

    #[command(subcommand, about = "Content performance analytics")]
    Content(content::ContentCommands),

    #[command(subcommand, about = "Traffic analytics")]
    Traffic(traffic::TrafficCommands),

    #[command(about = "Cost analytics; defaults to the spend summary")]
    Costs(costs::CostsArgs),
}

impl DescribeCommand for AnalyticsCommands {
    fn descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::FULL
            .with_skip_validation()
            .with_read_only()
            .with_local_export()
            .with_data_impact(DataImpact::Preserving)
    }
}

impl CliCommandGroup for AnalyticsCommands {
    const PARENT: &'static [&'static str] = &[];
    const NAME: &'static str = "analytics";
    const ABOUT: &'static str = "Analytics and metrics reporting";
    const DISPLAY_ORDER: usize = 50;

    fn db_url_routing(&self) -> DbUrlRouting {
        DbUrlRouting::Direct
    }

    fn run(self, ctx: &CommandContext) -> impl Future<Output = Result<()>> + Send {
        execute(self, ctx)
    }
}

systemprompt_cli::register_cli_command!(AnalyticsCommands);

pub async fn execute(command: AnalyticsCommands, ctx: &CommandContext) -> Result<()> {
    match command {
        AnalyticsCommands::Overview(args) => {
            let result = overview::execute_with_pool(
                with_resolved_export!(args, ctx),
                &ctx.database().await?,
                &ctx.cli,
            )
            .await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        AnalyticsCommands::Conversations(cmd) => conversations::execute(cmd, ctx).await,
        AnalyticsCommands::Agents(cmd) => agents::execute(cmd, ctx).await,
        AnalyticsCommands::Tools(cmd) => tools::execute(cmd, ctx).await,
        AnalyticsCommands::Requests(cmd) => requests::execute(cmd, ctx).await,
        AnalyticsCommands::Sessions(cmd) => sessions::execute(cmd, ctx).await,
        AnalyticsCommands::Content(cmd) => content::execute(cmd, ctx).await,
        AnalyticsCommands::Traffic(cmd) => traffic::execute(cmd, ctx).await,
        AnalyticsCommands::Costs(cmd) => costs::execute(cmd, ctx).await,
    }
}
