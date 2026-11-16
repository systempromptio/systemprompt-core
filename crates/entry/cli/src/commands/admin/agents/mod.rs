//! `admin agents` command group: manage A2A agent configurations and runtime.
//!
//! Exposes [`AgentsCommands`] covering the agent lifecycle — list, show,
//! validate, create, edit, delete, status, logs — plus A2A interaction
//! (registry discovery, message, task, tools) and direct `run`. Each variant
//! dispatches to a sibling module that returns a
//! [`systemprompt_extension::cli::CommandOutput`].
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
pub mod message_streaming;
pub mod process_stop;
pub mod registry;
pub mod run;
pub mod shared;
pub mod show;
mod status;
mod task;
mod tools;
pub mod validate;

use crate::kernel::KernelHost;
use anyhow::{Context, Result, bail};
use clap::Subcommand;

use systemprompt_extension::cli::DataImpact;
use systemprompt_extension::cli::CommandOutput;

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

pub async fn execute(command: AgentsCommands, host: &dyn KernelHost) -> Result<CommandOutput> {
    let result = match command {
        AgentsCommands::List(args) => list::execute(args, host).await.context("Failed to list agents")?,
        AgentsCommands::Show(args) => show::execute(args, host).await.context("Failed to show agent")?,
        AgentsCommands::Validate(args) => {
            let (result, valid) =
                validate::execute(&args, host).await.context("Failed to validate agents")?;
            if !valid {
                host.render(&result);
                bail!("Agent validation failed");
            }
            result
        },
        AgentsCommands::Create(args) => {
            create::execute(args, host).await.context("Failed to create agent")?
        },
        AgentsCommands::Edit(args) => edit::execute(&args, host).await.context("Failed to edit agent")?,
        AgentsCommands::Delete(args) => Box::pin(delete::execute(args, host))
            .await
            .context("Failed to delete agent")?,
        AgentsCommands::Status(args) => status::execute(args, host)
            .await
            .context("Failed to get agent status")?,
        AgentsCommands::Logs(args) => logs::execute(args, host)
            .await
            .context("Failed to get agent logs")?,
        AgentsCommands::Registry(args) => registry::execute(args, host)
            .await
            .context("Failed to get agent registry")?,
        AgentsCommands::Message(args) => Box::pin(message::execute(args, host))
            .await
            .context("Failed to send message to agent")?,
        AgentsCommands::Task(args) => task::execute(args, host)
            .await
            .context("Failed to get task details")?,
        AgentsCommands::Tools(args) => Box::pin(tools::execute(args, host))
            .await
            .context("Failed to list agent tools")?,
        AgentsCommands::Run(args) => {
            Box::pin(run::execute(args, host))
                .await
                .context("Failed to run agent")?;
            CommandOutput::rendered()
        },
    };
    Ok(result)
}

impl AgentsCommands {
    pub const fn data_impact(&self) -> DataImpact {
        match self {
            Self::Delete(_) => DataImpact::Destructive,
            Self::List(_)
            | Self::Show(_)
            | Self::Validate(_)
            | Self::Create(_)
            | Self::Edit(_)
            | Self::Status(_)
            | Self::Logs(_)
            | Self::Registry(_)
            | Self::Message(_)
            | Self::Task(_)
            | Self::Tools(_)
            | Self::Run(_) => DataImpact::Preserving,
        }
    }
}
