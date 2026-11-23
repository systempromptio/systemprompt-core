//! [`McpServiceObserver`]: a read-only view of the configured MCP servers'
//! state.
//!
//! It reports each enabled server's health and recorded process id and lists
//! the servers the service table records as running. It holds no spawner and
//! no secrets, so its holder can observe MCP servers but never start, stop,
//! restart or signal one. The `mcp` member provides it into the kernel state
//! for member commands, where health is port reachability
//! ([`HealthProbe::Listening`]): member code holds no health hop. The
//! [`McpOrchestrator`](super::McpOrchestrator) answers the same two
//! questions through one that performs the MCP handshake.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_config::paths::AppPaths;
use systemprompt_database::ServiceRepository;

use super::database::DatabaseService;
use super::health_probe::HealthProbe;
use super::monitoring::health::perform_health_check;
use super::monitoring::status::McpServiceStatus;
use super::registry::RegistryService;
use crate::McpServerConfig;
use crate::error::McpDomainResult;

/// Observes MCP server health and the recorded running set, read-only.
#[derive(Debug, Clone)]
pub struct McpServiceObserver {
    database: DatabaseService,
    registry: RegistryService,
    probe: HealthProbe,
}

impl McpServiceObserver {
    #[must_use]
    pub fn new(
        service_repo: ServiceRepository,
        app_paths: Arc<AppPaths>,
        registry: RegistryService,
    ) -> Self {
        Self {
            database: DatabaseService::new(service_repo, app_paths, registry.clone()),
            registry,
            probe: HealthProbe::Listening,
        }
    }

    pub(super) const fn over(
        database: DatabaseService,
        registry: RegistryService,
        probe: HealthProbe,
    ) -> Self {
        Self {
            database,
            registry,
            probe,
        }
    }

    pub async fn service_statuses(&self) -> McpDomainResult<Vec<McpServiceStatus>> {
        let servers = self.registry.get_enabled_servers()?;
        let mut statuses = Vec::with_capacity(servers.len());
        for server in &servers {
            let health = perform_health_check(server, &self.probe).await?;
            let pid = if server.is_external() {
                None
            } else {
                self.database
                    .get_service_by_name(&server.service_name())
                    .await?
                    .and_then(|info| super::database::stored_pid(info.pid))
            };
            statuses.push(McpServiceStatus::observed(server, &health, pid));
        }
        Ok(statuses)
    }

    #[must_use]
    pub const fn probe(&self) -> &HealthProbe {
        &self.probe
    }

    pub async fn running_servers(&self) -> McpDomainResult<Vec<McpServerConfig>> {
        self.database.get_running_servers().await
    }
}
