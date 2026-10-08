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

#[derive(Debug, Clone, Copy, Default)]
pub struct MonitoringService;

impl MonitoringService {
    pub const fn new() -> Self {
        Self
    }

    pub async fn check_health(
        &self,
        config: &McpServerConfig,
    ) -> McpDomainResult<health::HealthStatus> {
        health::check_service_health(config).await
    }

    pub async fn get_status_for_all(
        &self,
        servers: &[McpServerConfig],
    ) -> McpDomainResult<Vec<status::McpServiceStatus>> {
        status::get_all_service_status(servers).await
    }

    pub fn display_status(statuses: &[status::McpServiceStatus]) {
        status::display_service_status(statuses);
    }
}
