use anyhow::{Context, Result};
use clap::Args;
use std::sync::Arc;

use super::types::McpStatusOutput;
use crate::shared::CommandResult;
use crate::CliConfig;
use systemprompt_core_mcp::services::McpManager;
use systemprompt_loader::ConfigLoader;
use systemprompt_models::AppPaths;
use systemprompt_runtime::AppContext;

#[derive(Debug, Clone, Args)]
pub struct StatusArgs {
    #[arg(long, short, help = "Show detailed output including binary paths")]
    pub detailed: bool,

    #[arg(long, help = "Filter to specific server")]
    pub server: Option<String>,
}

pub async fn execute(
    args: StatusArgs,
    _config: &CliConfig,
) -> Result<CommandResult<McpStatusOutput>> {
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;
    let paths = AppPaths::get().context("Failed to get application paths")?;
    let bin_path = paths.build().bin().to_path_buf();

    let ctx = Arc::new(
        AppContext::new()
            .await
            .context("Failed to initialize application context")?,
    );

    let manager = McpManager::new(ctx).context("Failed to initialize MCP manager")?;
    let running_servers = manager
        .get_running_servers()
        .await
        .context("Failed to get running servers")?;

    let mut servers = Vec::new();

    for (name, deployment) in &services_config.mcp_servers {
        // Filter by server name if specified
        if let Some(ref filter) = args.server {
            if name != filter {
                continue;
            }
        }

        let running_info = running_servers.iter().find(|s| &s.name == name);
        let is_running = running_info.is_some();

        let binary_name = &deployment.binary;
        let release_path = bin_path
            .parent()
            .map(|p| p.join("release").join(binary_name))
            .filter(|p| p.exists());
        let debug_path = bin_path
            .parent()
            .map(|p| p.join("debug").join(binary_name))
            .filter(|p| p.exists());

        let pid = manager
            .get_service_info(name)
            .await
            .ok()
            .flatten()
            .and_then(|info| info.pid.map(|p| p as u32));

        let status_entry = super::types::McpStatusEntry {
            name: name.clone(),
            port: deployment.port,
            enabled: deployment.enabled,
            running: is_running,
            pid,
            binary: binary_name.clone(),
            release_binary: release_path.map(|p| {
                if args.detailed {
                    p.display().to_string()
                } else {
                    "exists".to_string()
                }
            }),
            debug_binary: debug_path.map(|p| {
                if args.detailed {
                    p.display().to_string()
                } else {
                    "exists".to_string()
                }
            }),
        };

        servers.push(status_entry);
    }

    servers.sort_by(|a, b| a.name.cmp(&b.name));

    let running_count = servers.iter().filter(|s| s.running).count();
    let enabled_count = servers.iter().filter(|s| s.enabled).count();

    let output = McpStatusOutput {
        servers,
        summary: super::types::McpStatusSummary {
            total: services_config.mcp_servers.len(),
            enabled: enabled_count,
            running: running_count,
        },
    };

    let columns = vec![
        "name".to_string(),
        "port".to_string(),
        "enabled".to_string(),
        "running".to_string(),
        "pid".to_string(),
        "release_binary".to_string(),
        "debug_binary".to_string(),
    ];

    Ok(CommandResult::table(output)
        .with_title("MCP Server Status")
        .with_columns(columns))
}
