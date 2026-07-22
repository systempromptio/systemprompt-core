//! `admin agents` command group: manage A2A agent configurations and runtime.
//!
//! Exposes [`AgentsCommands`] covering the agent lifecycle — list, show,
//! validate, create, edit, delete, status, logs — plus A2A interaction
//! (registry discovery, message, task, tools) and direct `run`. Each variant
//! dispatches to a sibling module that returns a
//! [`crate::shared::CommandOutput`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod types;

mod client;
pub mod create;
pub mod delete;
mod edit;
mod edit_apply;
mod list;
pub mod logs;
pub mod logs_db;
pub mod logs_disk;
mod message;
mod message_request;
mod message_streaming;
mod registry;
mod run;
pub mod shared;
pub mod show;
mod status;
mod task;
mod tools;
mod tools_mcp;
pub mod validate;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::context::CommandContext;
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum AgentsCommands {
    #[command(about = "List configured agents")]
    List(list::ListArgs),

    #[command(about = "Display agent configuration")]
    Show(show::ShowArgs),

    #[command(about = "Check agent configs for errors")]
    Validate(validate::ValidateArgs),

    #[command(about = "Create new agent")]
    Create(create::CreateArgs),

    #[command(about = "Edit agent configuration")]
    Edit(edit::EditArgs),

    #[command(about = "Delete an agent")]
    Delete(delete::DeleteArgs),

    #[command(about = "Show agent process status")]
    Status(status::StatusArgs),

    #[command(about = "View agent logs")]
    Logs(logs::LogsArgs),

    #[command(about = "Get running agents from gateway registry (A2A discovery)")]
    Registry(registry::RegistryArgs),

    #[command(about = "Send a message to an agent via A2A protocol")]
    Message(message::MessageArgs),

    #[command(about = "Get task details and response from an agent")]
    Task(task::TaskArgs),

    #[command(about = "List MCP tools available to an agent")]
    Tools(tools::ToolsArgs),

    #[command(about = "Run an agent server directly (bypasses orchestration)")]
    Run(run::RunArgs),
}

pub async fn execute(command: AgentsCommands, ctx: &CommandContext) -> Result<()> {
    let result = match command {
        AgentsCommands::List(args) => {
            list::execute(args, &ctx.cli).context("Failed to list agents")?
        },
        AgentsCommands::Show(args) => {
            show::execute(args, ctx.prompter(), &ctx.cli).context("Failed to show agent")?
        },
        AgentsCommands::Validate(args) => {
            validate::execute(&args, &ctx.cli).context("Failed to validate agents")?
        },
        AgentsCommands::Create(args) => {
            create::execute(args, ctx.prompter(), &ctx.cli).context("Failed to create agent")?
        },
        AgentsCommands::Edit(args) => {
            edit::execute(&args, ctx.prompter(), &ctx.cli).context("Failed to edit agent")?
        },
        AgentsCommands::Delete(args) => delete::execute(args, ctx.prompter(), &ctx.cli)
            .await
            .context("Failed to delete agent")?,
        AgentsCommands::Status(args) => status::execute(args, &ctx.cli)
            .await
            .context("Failed to get agent status")?,
        AgentsCommands::Logs(args) => logs::execute(args, ctx.prompter(), &ctx.cli)
            .await
            .context("Failed to get agent logs")?,
        AgentsCommands::Registry(args) => registry::execute(args, &ctx.cli)
            .await
            .context("Failed to get agent registry")?,
        AgentsCommands::Message(args) => message::execute(args, ctx)
            .await
            .context("Failed to send message to agent")?,
        AgentsCommands::Task(args) => task::execute(args, ctx)
            .await
            .context("Failed to get task details")?,
        AgentsCommands::Tools(args) => tools::execute(args, ctx)
            .await
            .context("Failed to list agent tools")?,
        AgentsCommands::Run(args) => {
            return run::execute(args).await.context("Failed to run agent");
        },
    };
    render_result(&result, &ctx.cli);
    Ok(())
}
