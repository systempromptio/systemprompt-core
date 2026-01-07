use anyhow::{Context, Result};
use clap::Subcommand;
use std::env;
use std::sync::Arc;
use systemprompt_core_logging::CliService;
use systemprompt_core_mcp::services::registry::RegistryManager;
use systemprompt_core_mcp::services::McpManager;
use systemprompt_runtime::{validate_system, AppContext};

#[derive(Subcommand)]
pub enum McpCommands {
    #[command(about = "List all MCP services and their status")]
    List,
    #[command(about = "Start MCP services")]
    Start { service: Option<String> },
    #[command(about = "Stop MCP services")]
    Stop { service: Option<String> },
    #[command(about = "Build MCP services")]
    Build { service: Option<String> },
    #[command(about = "Restart MCP services")]
    Restart { service: Option<String> },
    #[command(about = "Show status of MCP services")]
    Status,
    #[command(about = "Synchronize database state with actual running processes")]
    Sync,
    #[command(about = "Validate MCP connection and list tools")]
    Validate { service: String },
    #[command(about = "List enabled MCP package names for build scripts")]
    ListPackages,
}

pub async fn execute(cmd: McpCommands) -> Result<()> {
    env::set_var("SYSTEMPROMPT_NON_INTERACTIVE", "1");

    let ctx = Arc::new(
        AppContext::new()
            .await
            .context("Failed to initialize application context")?,
    );

    match &cmd {
        McpCommands::List | McpCommands::Status | McpCommands::ListPackages => {},
        _ => {
            CliService::info("Running system validation...");
            validate_system(&ctx)
                .await
                .context("System validation failed")?;
            CliService::success("System validation completed");
        },
    }

    let manager = McpManager::new(ctx).context("Failed to initialize MCP manager")?;

    match cmd {
        McpCommands::List => {
            manager.list_services().await?;
        },
        McpCommands::Start { service } => {
            manager.start_services(service).await?;
        },
        McpCommands::Stop { service } => {
            manager.stop_services(service).await?;
        },
        McpCommands::Build { service } => {
            manager.build_services(service).await?;
        },
        McpCommands::Restart { service } => {
            manager.restart_services(service).await?;
        },
        McpCommands::Status => {
            manager.show_status().await?;
        },
        McpCommands::Sync => {
            manager.sync_database_state().await?;
        },
        McpCommands::Validate { service } => {
            manager.validate_service(&service).await?;
        },
        McpCommands::ListPackages => {
            let servers = RegistryManager::get_enabled_servers()?;
            let packages: Vec<_> = servers.iter().map(|s| s.name.clone()).collect();
            #[allow(clippy::print_stdout)]
            {
                println!("{}", packages.join(" "));
            }
        },
    }

    Ok(())
}
