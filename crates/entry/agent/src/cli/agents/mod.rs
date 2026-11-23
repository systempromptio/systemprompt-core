//! `admin agents`: authoring, inspecting and messaging configured agents.
//!
//! `agent-http` contributes the group under the kernel's `admin` group with
//! the `services-tree` and `caller-session` grants:
//!
//! | command | reads |
//! |---|---|
//! | `list`, `show` | `ServicesView` |
//! | `validate` | `ServicesView`, the presence-only `Arc<dyn SecretPresence>` |
//! | `create`, `edit`, `delete` | the services tree (YAML under its root) |
//! | `status` | the open `Arc<AgentStatusView>` |
//! | `logs` | the database handle, `ServicesView`, `Arc<AppPaths>`, the child supervisor |
//! | `registry` | the services tree's public API URL |
//! | `message`, `task` | the caller's own session, presented to the platform API only |
//!
//! None of them starts, stops or signals an agent: `delete` refuses a
//! running agent and names `infra services stop`, and `admin agent-host run`
//! (the spawned agent process) is the kernel's.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
pub mod registry;
pub mod shared;
pub mod show;
mod status;
mod task;
pub mod types;
pub mod validate;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Subcommand;
use systemprompt_extension::cli::{
    CliCommandGroup, CommandDescriptor, CommandError, CommandHost, CommandOutput, DataImpact,
    DescribeCommand,
};
use systemprompt_loader::ConfigLoader;
use systemprompt_manifest::ServicesConfig;
use systemprompt_manifest::services::ServicesView;
use systemprompt_models::artifacts::NoticeLine;

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
}

pub(crate) async fn services_config(host: &dyn CommandHost) -> Result<Arc<ServicesConfig>> {
    Ok(host.resolved().await?.get::<ServicesView>()?.config())
}

pub(crate) fn services_root(host: &dyn CommandHost) -> Result<PathBuf> {
    Ok(host.services_tree()?.root().to_path_buf())
}

fn services_config_path(host: &dyn CommandHost) -> Result<PathBuf> {
    Ok(services_root(host)?.join("config").join("config.yaml"))
}

pub(crate) fn reload_services_config(host: &dyn CommandHost) -> Result<ServicesConfig> {
    Ok(ConfigLoader::new(services_config_path(host)?).load()?)
}

pub(crate) fn services_config_as_written(host: &dyn CommandHost) -> Result<ServicesConfig> {
    ConfigLoader::new(services_config_path(host)?)
        .verbatim()
        .load()
        .context("Failed to load services configuration")
}

pub(crate) fn notice(host: &dyn CommandHost, level: &str, text: impl Into<String>) {
    host.render(&CommandOutput::message(vec![NoticeLine::new(level, text)]));
}

pub async fn execute(command: AgentsCommands, host: &dyn CommandHost) -> Result<CommandOutput> {
    let result = match command {
        AgentsCommands::List(args) => list::execute(args, host)
            .await
            .context("Failed to list agents")?,
        AgentsCommands::Show(args) => show::execute(args, host)
            .await
            .context("Failed to show agent")?,
        AgentsCommands::Validate(args) => {
            let (result, valid) = validate::execute(&args, host)
                .await
                .context("Failed to validate agents")?;
            if !valid {
                return Err(
                    CommandError::new(anyhow::anyhow!("Agent validation failed"))
                        .with_output(result)
                        .into(),
                );
            }
            result
        },
        AgentsCommands::Create(args) => {
            create::execute(args, host).context("Failed to create agent")?
        },
        AgentsCommands::Edit(args) => edit::execute(&args, host).context("Failed to edit agent")?,
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
        AgentsCommands::Task(args) => Box::pin(task::execute(args, host))
            .await
            .context("Failed to get task details")?,
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
            | Self::Task(_) => DataImpact::Preserving,
        }
    }
}

impl DescribeCommand for AgentsCommands {
    fn descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::FULL.with_data_impact(self.data_impact())
    }
}

impl CliCommandGroup for AgentsCommands {
    const NAME: &'static str = "agents";
    const ABOUT: &'static str = "Agent management";

    async fn run(self, host: &dyn CommandHost) -> Result<CommandOutput, CommandError> {
        execute(self, host).await.map_err(crate::cli::command_error)
    }
}
