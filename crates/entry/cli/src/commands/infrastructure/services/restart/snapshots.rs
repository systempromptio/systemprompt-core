//! Service snapshots the batch restart plans over: every agent and managed
//! MCP server with its enabled flag and, when probed, its health, read from
//! the supervised process sets.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use std::sync::Arc;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::{ServiceSnapshot, ServiceType};
use systemprompt_traits::{ServiceDetail, ServiceProcessKind, StatusDepth};

use super::super::lifecycle;

pub(super) async fn agent_snapshots(ctx: &Arc<AppContext>) -> Result<Vec<ServiceSnapshot>> {
    let statuses =
        lifecycle::statuses_of(ctx, ServiceProcessKind::Agent, StatusDepth::Listed).await?;
    Ok(statuses
        .into_iter()
        .map(|status| {
            let (enabled, failed) = match &status.detail {
                ServiceDetail::Agent { enabled, failure } => (*enabled, failure.is_some()),
                ServiceDetail::Mcp { .. } => (true, false),
            };
            ServiceSnapshot {
                service_type: ServiceType::Agent,
                id: status.name.to_string(),
                name: status.name.to_string(),
                enabled,
                healthy: !failed,
            }
        })
        .collect())
}

pub(super) async fn mcp_snapshots(
    ctx: &Arc<AppContext>,
    probe_health: bool,
) -> Result<Vec<ServiceSnapshot>> {
    let depth = if probe_health {
        StatusDepth::Probed
    } else {
        StatusDepth::Listed
    };
    let statuses = lifecycle::statuses_of(ctx, ServiceProcessKind::McpServer, depth).await?;
    Ok(statuses
        .into_iter()
        .filter(|status| !matches!(status.detail, ServiceDetail::Mcp { remote: true, .. }))
        .map(|status| ServiceSnapshot {
            service_type: ServiceType::Mcp,
            id: status.name.to_string(),
            name: status.name.to_string(),
            enabled: true,
            healthy: !probe_health || status.health.is_up(),
        })
        .collect())
}
