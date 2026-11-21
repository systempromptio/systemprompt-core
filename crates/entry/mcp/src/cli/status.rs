//! `plugins mcp status` command: each configured server's health, recorded
//! process id and build binaries, read through the `mcp` member's read-only
//! observer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use clap::Args;
use systemprompt_extension::cli::{CommandHost, CommandOutput};
use systemprompt_manifest::ServicesConfig;
use systemprompt_mcp::{HealthStatus, McpServiceStatus};
use systemprompt_models::mcp::McpServerType;

use super::list::binary_path;
use super::types::{McpStatusEntry, McpStatusOutput, McpStatusSummary};
use super::{build_target_dir, observer, services_config};

#[derive(Debug, Clone, Args)]
pub struct StatusArgs {
    #[arg(long, short, help = "Show detailed output including binary paths")]
    pub detailed: bool,

    #[arg(long, help = "Filter to specific server")]
    pub server: Option<String>,
}

pub(super) async fn execute(args: StatusArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let services_config = services_config(host)
        .await
        .context("Failed to load services configuration")?;
    let target_dir = build_target_dir(host).await?;

    let statuses = observer(host)
        .await?
        .service_statuses()
        .await
        .context("Failed to get MCP service statuses")?;
    let status_by_name: HashMap<&str, &McpServiceStatus> =
        statuses.iter().map(|s| (s.name.as_str(), s)).collect();

    let mut servers = collect_status_entries(
        &args,
        &services_config,
        &status_by_name,
        target_dir.as_deref(),
    );

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
    target_dir: Option<&Path>,
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

        let entry =
            match deployment.server_type {
                McpServerType::External => McpStatusEntry {
                    name: name.clone(),
                    server_type: McpServerType::External.as_str().to_owned(),
                    port: None,
                    enabled: deployment.enabled,
                    running,
                    health,
                    pid: None,
                    endpoint: status.and_then(|s| s.endpoint.clone()),
                    binary: None,
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
                    release_binary: deployment.binary.as_deref().and_then(|binary| {
                        binary_display(target_dir, "release", binary, args.detailed)
                    }),
                    debug_binary: deployment.binary.as_deref().and_then(|binary| {
                        binary_display(target_dir, "debug", binary, args.detailed)
                    }),
                },
            };

        servers.push(entry);
    }

    servers
}

fn binary_display(
    target_dir: Option<&Path>,
    profile: &str,
    binary_name: &str,
    detailed: bool,
) -> Option<String> {
    binary_path(target_dir, binary_name, profile).map(|p| {
        if detailed {
            p.display().to_string()
        } else {
            "exists".to_owned()
        }
    })
}
