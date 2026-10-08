//! Aggregated service-status reporting for MCP servers.
//!
//! [`McpServiceStatus`] is the one per-server status model: the health
//! verdict of a live probe plus where the server is reachable. The state a
//! probe implies is reported as the registry's shared [`ServiceStatus`], never
//! as a parallel string vocabulary.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::McpServerConfig;
use crate::error::McpDomainResult;
use crate::services::monitoring::health::{HealthCheckResult, HealthStatus, perform_health_check};
use systemprompt_identifiers::McpServerId;
use systemprompt_manifest::services::ServiceStatus;
use systemprompt_models::mcp::McpServerType;

#[derive(Debug, Clone)]
pub struct McpServiceStatus {
    pub name: McpServerId,
    pub server_type: McpServerType,
    pub port: Option<u16>,
    pub endpoint: Option<String>,
    pub health: HealthStatus,
    pub pid: Option<u32>,
    pub tools_count: Option<usize>,
    pub latency_ms: Option<u32>,
    pub auth_required: bool,
}

impl McpServiceStatus {
    #[must_use]
    pub fn observed(config: &McpServerConfig, check: &HealthCheckResult, pid: Option<u32>) -> Self {
        Self {
            health: check.status,
            pid,
            tools_count: check.details.tools_available,
            latency_ms: Some(check.latency_ms),
            ..Self::unreachable(config)
        }
    }

    #[must_use]
    pub fn unreachable(config: &McpServerConfig) -> Self {
        let external = config.is_external();
        Self {
            name: McpServerId::new(config.name.as_str()),
            server_type: config.server_type,
            port: if external { None } else { config.port },
            endpoint: external.then(|| config.remote_endpoint.clone()),
            health: HealthStatus::Unhealthy,
            pid: None,
            tools_count: None,
            latency_ms: None,
            auth_required: config.oauth.required,
        }
    }

    #[must_use]
    pub const fn observed_state(&self) -> ServiceStatus {
        match self.health {
            HealthStatus::Healthy | HealthStatus::Degraded => ServiceStatus::Running,
            HealthStatus::Unhealthy => ServiceStatus::Stopped,
            HealthStatus::Unknown => ServiceStatus::Error,
        }
    }
}

pub async fn get_all_service_status(
    servers: &[McpServerConfig],
) -> McpDomainResult<Vec<McpServiceStatus>> {
    let mut statuses = Vec::with_capacity(servers.len());
    for server in servers {
        statuses.push(get_service_status(server).await);
    }
    Ok(statuses)
}

async fn get_service_status(config: &McpServerConfig) -> McpServiceStatus {
    match perform_health_check(config).await {
        Ok(check) => McpServiceStatus::observed(config, &check, None),
        Err(e) => {
            tracing::debug!(service = %config.name, error = %e, "Health check failed; reporting service unreachable");
            McpServiceStatus::unreachable(config)
        },
    }
}

pub fn display_service_status(statuses: &[McpServiceStatus]) {
    if statuses.is_empty() {
        tracing::info!("No MCP services configured");
        return;
    }

    let count = |state: ServiceStatus| {
        statuses
            .iter()
            .filter(|s| s.observed_state() == state)
            .count()
    };
    let running_count = count(ServiceStatus::Running);
    let error_count = count(ServiceStatus::Error);

    tracing::info!(
        running = running_count,
        error = error_count,
        total = statuses.len(),
        "MCP services status"
    );
}
