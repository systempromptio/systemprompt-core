//! Top-level `analytics` command group spanning all metric domains.
//!
//! [`AnalyticsCommands`] routes to the per-domain subcommand trees (overview,
//! conversations, agents, tools, requests, sessions, content, traffic, costs).
//! Each variant dispatches to its module's `execute` against the invocation's
//! [`CommandContext`]. The [`shared`] module holds the time-range and output
//! formatting helpers used across all domains.

pub mod agents;
pub mod content;
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

use crate::context::CommandContext;
use crate::shared::render_result;

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

    #[command(subcommand, about = "AI request analytics")]
    Requests(requests::RequestsCommands),

    #[command(subcommand, about = "Session analytics")]
    Sessions(sessions::SessionsCommands),

    #[command(subcommand, about = "Content performance analytics")]
    Content(content::ContentCommands),

    #[command(subcommand, about = "Traffic analytics")]
    Traffic(traffic::TrafficCommands),

    #[command(subcommand, about = "Cost analytics")]
    Costs(costs::CostsCommands),
}

pub async fn execute(command: AnalyticsCommands, ctx: &CommandContext) -> Result<()> {
    match command {
        AnalyticsCommands::Overview(args) => {
            let result =
                overview::execute_with_pool(args, &ctx.database().await?, &ctx.cli).await?;
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
