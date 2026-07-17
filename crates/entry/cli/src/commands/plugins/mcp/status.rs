//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Args;

use super::types::{McpStatusEntry, McpStatusOutput, McpStatusSummary};
use crate::CliConfig;
use crate::shared::CommandOutput;
use systemprompt_loader::ConfigLoader;
use systemprompt_mcp::services::McpOrchestrator;
use systemprompt_mcp::{HealthStatus, McpServiceStatus};
use systemprompt_models::ServicesConfig;
use systemprompt_models::mcp::McpServerType;
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
    let statuses = manager
        .service_statuses()
        .await
        .context("Failed to get MCP service statuses")?;
    let status_by_name: HashMap<&str, &McpServiceStatus> =
        statuses.iter().map(|s| (s.name.as_str(), s)).collect();

    let mut servers = collect_status_entries(&args, &services_config, &status_by_name, &bin_path);

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
            "server_type",
            "port",
            "enabled",
            "running",
            "health",
            "pid",
            "endpoint",
            "release_binary",
            "debug_binary",
        ],
        &output.servers,
    )
    .with_title("MCP Server Status"))
}

fn collect_status_entries(
    args: &StatusArgs,
    services_config: &ServicesConfig,
    status_by_name: &HashMap<&str, &McpServiceStatus>,
    bin_path: &Path,
) -> Vec<McpStatusEntry> {
    let mut servers = Vec::new();

    for (name, deployment) in &services_config.mcp_servers {
        if let Some(ref filter) = args.server
            && name != filter
        {
            continue;
        }

        let status = status_by_name.get(name.as_str());
        let health = status.map_or_else(|| "unknown".to_owned(), |s| s.health.as_str().to_owned());
        let running = status
            .is_some_and(|s| matches!(s.health, HealthStatus::Healthy | HealthStatus::Degraded));
        let pid = status.and_then(|s| s.pid);

        let entry = match deployment.server_type {
            McpServerType::External => McpStatusEntry {
                name: name.clone(),
                server_type: McpServerType::External.as_str().to_owned(),
                port: 0,
                enabled: deployment.enabled,
                running,
                health,
                pid: None,
                endpoint: status.and_then(|s| s.endpoint.clone()),
                binary: String::new(),
                release_binary: None,
                debug_binary: None,
            },
            McpServerType::Internal => McpStatusEntry {
                name: name.clone(),
                server_type: McpServerType::Internal.as_str().to_owned(),
                port: deployment.port,
                enabled: deployment.enabled,
                running,
                health,
                pid,
                endpoint: None,
                binary: deployment.binary.clone(),
                release_binary: binary_display(
                    bin_path,
                    "release",
                    &deployment.binary,
                    args.detailed,
                ),
                debug_binary: binary_display(bin_path, "debug", &deployment.binary, args.detailed),
            },
        };

        servers.push(entry);
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
