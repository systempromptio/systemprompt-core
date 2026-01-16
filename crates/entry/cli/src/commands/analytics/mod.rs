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
use systemprompt_runtime::DatabaseContext;

use crate::CliConfig;

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

pub async fn execute(command: AnalyticsCommands, config: &CliConfig) -> Result<()> {
    match command {
        AnalyticsCommands::Overview(args) => overview::execute(args, config).await,
        AnalyticsCommands::Conversations(cmd) => conversations::execute(cmd, config).await,
        AnalyticsCommands::Agents(cmd) => agents::execute(cmd, config).await,
        AnalyticsCommands::Tools(cmd) => tools::execute(cmd, config).await,
        AnalyticsCommands::Requests(cmd) => requests::execute(cmd, config).await,
        AnalyticsCommands::Sessions(cmd) => sessions::execute(cmd, config).await,
        AnalyticsCommands::Content(cmd) => content::execute(cmd, config).await,
        AnalyticsCommands::Traffic(cmd) => traffic::execute(cmd, config).await,
        AnalyticsCommands::Costs(cmd) => costs::execute(cmd, config).await,
    }
}

pub async fn execute_with_db(
    command: AnalyticsCommands,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    match command {
        AnalyticsCommands::Overview(args) => {
            overview::execute_with_pool(args, db_ctx, config).await
        },
        AnalyticsCommands::Conversations(cmd) => {
            conversations::execute_with_pool(cmd, db_ctx, config).await
        },
        AnalyticsCommands::Agents(cmd) => agents::execute_with_pool(cmd, db_ctx, config).await,
        AnalyticsCommands::Tools(cmd) => tools::execute_with_pool(cmd, db_ctx, config).await,
        AnalyticsCommands::Requests(cmd) => requests::execute_with_pool(cmd, db_ctx, config).await,
        AnalyticsCommands::Sessions(cmd) => sessions::execute_with_pool(cmd, db_ctx, config).await,
        AnalyticsCommands::Content(cmd) => content::execute_with_pool(cmd, db_ctx, config).await,
        AnalyticsCommands::Traffic(cmd) => traffic::execute_with_pool(cmd, db_ctx, config).await,
        AnalyticsCommands::Costs(cmd) => costs::execute_with_pool(cmd, db_ctx, config).await,
    }
}
