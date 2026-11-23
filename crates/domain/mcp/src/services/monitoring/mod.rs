//! Health and status monitoring for running MCP servers.
//!
//! Health probes, proxy reachability checks, and aggregated per-service
//! status snapshots.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod health;
pub mod proxy_health;
pub mod status;

use crate::McpServerConfig;
use crate::error::McpDomainResult;
use crate::services::health_probe::HealthProbe;

#[derive(Debug, Clone)]
pub struct MonitoringService {
    probe: HealthProbe,
}

impl MonitoringService {
    pub const fn new(probe: HealthProbe) -> Self {
        Self { probe }
    }

    pub const fn probe(&self) -> &HealthProbe {
        &self.probe
    }

    pub async fn check_health(
        &self,
        config: &McpServerConfig,
    ) -> McpDomainResult<health::HealthStatus> {
        health::check_service_health(config, &self.probe).await
    }

    pub async fn get_status_for_all(
        &self,
        servers: &[McpServerConfig],
    ) -> McpDomainResult<Vec<status::McpServiceStatus>> {
        status::get_all_service_status(servers, &self.probe).await
    }

    pub fn display_status(statuses: &[status::McpServiceStatus]) {
        status::display_service_status(statuses);
    }
}
