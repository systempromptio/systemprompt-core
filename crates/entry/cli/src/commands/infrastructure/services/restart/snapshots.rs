//! Service snapshots the batch restart plans over: every agent and managed
//! MCP server with its enabled flag and, when probed, its health.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_agent::services::agent_orchestration::{AgentOrchestrator, AgentStatus};
use systemprompt_agent::services::registry::AgentRegistry;
use systemprompt_mcp::HealthStatus;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::{ServiceSnapshot, ServiceType};

pub(super) async fn agent_snapshots(
    orchestrator: &AgentOrchestrator,
) -> Result<Vec<ServiceSnapshot>> {
    let agent_registry = AgentRegistry::new()?;
    let all_agents = orchestrator.list_all().await?;

    let mut snapshots = Vec::with_capacity(all_agents.len());
    for (agent_id, status) in &all_agents {
        let Ok(agent_config) = agent_registry.get_agent(agent_id.as_str()).await else {
            continue;
        };

        snapshots.push(ServiceSnapshot {
            service_type: ServiceType::Agent,
            id: agent_id.to_string(),
            name: agent_config.name,
            enabled: agent_config.enabled,
            healthy: !matches!(status, AgentStatus::Failed { .. }),
        });
    }
    Ok(snapshots)
}

pub(super) async fn mcp_snapshots(
    ctx: &Arc<AppContext>,
    probe_health: bool,
) -> Result<Vec<ServiceSnapshot>> {
    ctx.mcp_registry().validate()?;
    let servers = ctx.mcp_registry().get_managed_servers()?;

    let health_by_name: HashMap<String, HealthStatus> = if probe_health {
        let manager = systemprompt_mcp::services::McpOrchestrator::new(
            (**ctx.service_repository()).clone(),
            Arc::clone(ctx.app_paths_arc()),
            ctx.mcp_registry().clone(),
        )?;
        manager
            .service_statuses()
            .await?
            .into_iter()
            .map(|status| (String::from(status.name), status.health))
            .collect()
    } else {
        HashMap::new()
    };

    let mut snapshots = Vec::with_capacity(servers.len());
    for server in servers {
        let healthy = if probe_health {
            health_by_name
                .get(&server.name)
                .is_some_and(|h| matches!(h, HealthStatus::Healthy | HealthStatus::Degraded))
        } else {
            true
        };

        snapshots.push(ServiceSnapshot {
            service_type: ServiceType::Mcp,
            id: server.name.clone(),
            name: server.name.clone(),
            enabled: true,
            healthy,
        });
    }
    Ok(snapshots)
}
