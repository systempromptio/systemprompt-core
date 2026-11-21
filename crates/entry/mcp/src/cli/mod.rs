//! `plugins mcp` command group: observe and exercise configured MCP servers.
//!
//! `mcp-http` contributes the group under the kernel's `plugins` group with
//! the `caller-session` grant. Every command preserves data. The commands
//! reach kernel-held material only by type through the member's resolved
//! state and that one grant:
//!
//! | command | reads |
//! |---|---|
//! | `list` | `ServicesView`, `Arc<AppPaths>` (build binaries) |
//! | `list-packages` | `Arc<dyn McpRegistryProvider>` |
//! | `status` | `ServicesView`, `Arc<McpServiceObserver>`, `Arc<AppPaths>` |
//! | `validate` | `ServicesView`, `Arc<McpServiceObserver>` |
//! | `tools`, `call` | `ServicesView`, `Arc<McpServiceObserver>`, the caller's own session |
//! | `logs` | `ServicesView`, the database handle, `Arc<AppPaths>`, the child supervisor |
//!
//! None of them starts, stops, restarts or signals a server: `tools` and
//! `call` connect only to a server the service table records as running, and
//! `admin mcp` and `infra services` remain the kernel's control surface.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod call;
pub mod call_client;
mod disk_logs;
pub mod list;
mod list_packages;
pub mod logs;
pub mod logs_db;
mod logs_disk;
mod status;
mod tools;
mod tools_schema;
pub mod types;
pub mod validate;
mod validate_output;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Subcommand;
use systemprompt_config::paths::AppPaths;
use systemprompt_extension::cli::{
    CliCommandGroup, CommandDescriptor, CommandError, CommandHost, CommandOutput, DataImpact,
    DescribeCommand,
};
use systemprompt_manifest::ServicesConfig;
use systemprompt_manifest::services::ServicesView;
use systemprompt_mcp::services::McpServiceObserver;
use systemprompt_traits::McpRegistryProvider;

async fn services_config(host: &dyn CommandHost) -> Result<Arc<ServicesConfig>> {
    Ok(host.resolved().await?.get::<ServicesView>()?.config())
}

async fn observer(host: &dyn CommandHost) -> Result<Arc<McpServiceObserver>> {
    Ok(host.resolved().await?.get::<Arc<McpServiceObserver>>()?)
}

async fn registry(host: &dyn CommandHost) -> Result<Arc<dyn McpRegistryProvider>> {
    Ok(host
        .resolved()
        .await?
        .get::<Arc<dyn McpRegistryProvider>>()?)
}

async fn app_paths(host: &dyn CommandHost) -> Result<Arc<AppPaths>> {
    Ok(host.resolved().await?.get::<Arc<AppPaths>>()?)
}

async fn build_target_dir(host: &dyn CommandHost) -> Result<Option<PathBuf>> {
    Ok(app_paths(host)
        .await?
        .build()
        .bin()
        .parent()
        .map(Path::to_path_buf))
}

#[derive(Debug, Subcommand)]
pub enum McpCommands {
    #[command(about = "List configured MCP servers")]
    List(list::ListArgs),

    #[command(about = "Show MCP server runtime status")]
    Status(status::StatusArgs),

    #[command(about = "Validate MCP server configurations")]
    Validate(validate::ValidateArgs),

    #[command(about = "Tail logs for an MCP server")]
    Logs(logs::LogsArgs),

    #[command(about = "List discovered MCP packages from the registry")]
    ListPackages(list_packages::ListPackagesArgs),

    #[command(about = "List tools exposed by running MCP servers")]
    Tools(tools::ToolsArgs),

    #[command(about = "Invoke a tool on a running MCP server")]
    Call(call::CallArgs),
}

pub async fn execute(command: McpCommands, host: &dyn CommandHost) -> Result<CommandOutput> {
    match command {
        McpCommands::List(args) => list::execute(args, host)
            .await
            .context("Failed to list MCP servers"),
        McpCommands::Status(args) => status::execute(args, host)
            .await
            .context("Failed to get MCP server status"),
        McpCommands::Validate(args) => {
            let (result, valid) = Box::pin(validate::execute(args, host))
                .await
                .context("Failed to validate MCP server")?;
            if valid {
                return Ok(result);
            }
            Err(anyhow::Error::new(
                CommandError::new(anyhow::anyhow!("MCP server validation failed"))
                    .with_output(result),
            ))
        },
        McpCommands::Logs(args) => logs::execute(args, host)
            .await
            .context("Failed to get MCP server logs"),
        McpCommands::ListPackages(args) => list_packages::execute(args, host)
            .await
            .context("Failed to list MCP packages"),
        McpCommands::Tools(args) => Box::pin(tools::execute(args, host))
            .await
            .context("Failed to list MCP tools"),
        McpCommands::Call(args) => Box::pin(call::execute(args, host)).await,
    }
}

impl McpCommands {
    pub const fn data_impact(&self) -> DataImpact {
        match self {
            Self::List(_)
            | Self::Status(_)
            | Self::Validate(_)
            | Self::Logs(_)
            | Self::ListPackages(_)
            | Self::Tools(_)
            | Self::Call(_) => DataImpact::Preserving,
        }
    }
}

impl DescribeCommand for McpCommands {
    fn descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::FULL.with_data_impact(self.data_impact())
    }
}

impl CliCommandGroup for McpCommands {
    const NAME: &'static str = "mcp";
    const ABOUT: &'static str = "MCP server management";

    async fn run(self, host: &dyn CommandHost) -> Result<CommandOutput, CommandError> {
        execute(self, host)
            .await
            .map_err(|error| match error.downcast::<CommandError>() {
                Ok(failed) => failed,
                Err(error) => CommandError::new(error),
            })
    }
}
