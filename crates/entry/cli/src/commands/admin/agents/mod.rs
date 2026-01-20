pub mod types;

mod create;
mod delete;
mod edit;
mod list;
mod logs;
mod message;
mod registry;
mod run;
mod shared;
mod show;
mod status;
mod task;
mod tools;
mod validate;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::cli_settings::get_global_config;
use crate::shared::render_result;
use crate::CliConfig;

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

    #[command(about = "Run an agent server (internal use)", hide = true)]
    Run(run::RunArgs),
}

pub async fn execute(command: AgentsCommands) -> Result<()> {
    let config = get_global_config();
    execute_with_config(command, &config).await
}

pub async fn execute_with_config(command: AgentsCommands, config: &CliConfig) -> Result<()> {
    match command {
        AgentsCommands::List(args) => {
            let result = list::execute(args, config).context("Failed to list agents")?;
            render_result(&result);
            Ok(())
        },
        AgentsCommands::Show(args) => {
            let result = show::execute(args, config).context("Failed to show agent")?;
            render_result(&result);
            Ok(())
        },
        AgentsCommands::Validate(args) => {
            let result = validate::execute(&args, config).context("Failed to validate agents")?;
            render_result(&result);
            Ok(())
        },
        AgentsCommands::Create(args) => {
            let result = create::execute(args, config).context("Failed to create agent")?;
            render_result(&result);
            Ok(())
        },
        AgentsCommands::Edit(args) => {
            let result = edit::execute(args, config).context("Failed to edit agent")?;
            render_result(&result);
            Ok(())
        },
        AgentsCommands::Delete(args) => {
            let result = delete::execute(args, config).context("Failed to delete agent")?;
            render_result(&result);
            Ok(())
        },
        AgentsCommands::Status(args) => {
            let result = status::execute(args, config)
                .await
                .context("Failed to get agent status")?;
            render_result(&result);
            Ok(())
        },
        AgentsCommands::Logs(args) => {
            let result = logs::execute(args, config)
                .await
                .context("Failed to get agent logs")?;
            render_result(&result);
            Ok(())
        },
        AgentsCommands::Registry(args) => {
            let result = registry::execute(args, config)
                .await
                .context("Failed to get agent registry")?;
            render_result(&result);
            Ok(())
        },
        AgentsCommands::Message(args) => {
            let result = message::execute(args, config)
                .await
                .context("Failed to send message to agent")?;
            render_result(&result);
            Ok(())
        },
        AgentsCommands::Task(args) => {
            let result = task::execute(args, config)
                .await
                .context("Failed to get task details")?;
            render_result(&result);
            Ok(())
        },
        AgentsCommands::Tools(args) => {
            let result = tools::execute(args, config)
                .await
                .context("Failed to list agent tools")?;
            render_result(&result);
            Ok(())
        },
        AgentsCommands::Run(args) => {
            run::execute(args).await.context("Failed to run agent")?;
            Ok(())
        },
    }
}
