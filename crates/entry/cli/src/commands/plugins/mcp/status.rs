use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Args;

use super::types::{McpStatusEntry, McpStatusOutput, McpStatusSummary};
use crate::CliConfig;
use crate::shared::CommandOutput;
use systemprompt_loader::ConfigLoader;
use systemprompt_mcp::services::McpOrchestrator;
use systemprompt_models::{McpServerConfig, ServicesConfig};
use systemprompt_runtime::AppContext;

#[derive(Debug, Clone, Args)]
pub struct StatusArgs {
    #[arg(long, short, help = "Show detailed output including binary paths")]
    pub detailed: bool,

    #[arg(long, help = "Filter to specific server")]
    pub server: Option<String>,
}

pub(super) async fn execute(args: StatusArgs, _config: &CliConfig) -> Result<CommandOutput> {
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;

    let ctx = AppContext::new()
        .await
        .context("Failed to initialize application context")?;

    let bin_path = ctx.app_paths().build().bin().to_path_buf();

    let manager = McpOrchestrator::new(
        Arc::clone(ctx.db_pool()),
        Arc::clone(ctx.app_paths_arc()),
        ctx.mcp_registry().clone(),
    )
    .context("Failed to initialize MCP manager")?;
    let running_servers = manager
        .get_running_servers()
        .await
        .context("Failed to get running servers")?;

    let mut servers = collect_status_entries(
        &args,
        &services_config,
        &manager,
        &running_servers,
        &bin_path,
    )
    .await;

    servers.sort_by(|a, b| a.name.cmp(&b.name));

    let running_count = servers.iter().filter(|s| s.running).count();
    let enabled_count = servers.iter().filter(|s| s.enabled).count();

    let output = McpStatusOutput {
        servers,
        summary: McpStatusSummary {
            total: services_config.mcp_servers.len(),
            enabled: enabled_count,
            running: running_count,
        },
    };

    Ok(CommandOutput::table_of(
        vec![
            "name",
            "port",
            "enabled",
            "running",
            "pid",
            "release_binary",
            "debug_binary",
        ],
        &output.servers,
    )
    .with_title("MCP Server Status"))
}

async fn collect_status_entries(
    args: &StatusArgs,
    services_config: &ServicesConfig,
    manager: &McpOrchestrator,
    running_servers: &[McpServerConfig],
    bin_path: &Path,
) -> Vec<McpStatusEntry> {
    let mut servers = Vec::new();

    for (name, deployment) in &services_config.mcp_servers {
        if let Some(ref filter) = args.server
            && name != filter
        {
            continue;
        }

        let is_running = running_servers.iter().any(|s| &s.name == name);

        let pid = manager
            .get_service_info(name)
            .await
            .ok()
            .flatten()
            .and_then(|info| info.pid.map(|p| p as u32));

        servers.push(McpStatusEntry {
            name: name.clone(),
            port: deployment.port,
            enabled: deployment.enabled,
            running: is_running,
            pid,
            binary: deployment.binary.clone(),
            release_binary: binary_display(bin_path, "release", &deployment.binary, args.detailed),
            debug_binary: binary_display(bin_path, "debug", &deployment.binary, args.detailed),
        });
    }

    servers
}

fn binary_display(
    bin_path: &Path,
    profile: &str,
    binary_name: &str,
    detailed: bool,
) -> Option<String> {
    bin_path
        .parent()
        .map(|p| p.join(profile).join(binary_name))
        .filter(|p| p.exists())
        .map(|p| {
            if detailed {
                p.display().to_string()
            } else {
                "exists".to_owned()
            }
        })
}
