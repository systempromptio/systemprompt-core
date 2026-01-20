pub mod types;

mod capabilities;
mod config;
mod list;
mod run;
mod show;
mod validate;

pub mod mcp;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::shared::render_result;
use crate::CliConfig;

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

    #[command(subcommand, about = "List capabilities across all extensions")]
    Capabilities(capabilities::CapabilitiesCommands),

    #[command(subcommand, about = "MCP server management")]
    Mcp(mcp::McpCommands),
}

pub async fn execute(cmd: PluginsCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        PluginsCommands::List(args) => {
            render_result(&list::execute(&args, config));
            Ok(())
        },
        PluginsCommands::Show(args) => {
            let result = show::execute(&args, config).context("Failed to show extension")?;
            render_result(&result);
            Ok(())
        },
        PluginsCommands::Run(args) => run::execute(args, config).await,
        PluginsCommands::Validate(args) => {
            render_result(&validate::execute(&args, config));
            Ok(())
        },
        PluginsCommands::Config(args) => {
            let result =
                config::execute(&args, config).context("Failed to get extension config")?;
            render_result(&result);
            Ok(())
        },
        PluginsCommands::Capabilities(cmd) => {
            capabilities::execute(cmd, config);
            Ok(())
        },
        PluginsCommands::Mcp(cmd) => mcp::execute_with_config(cmd, config).await,
    }
}
