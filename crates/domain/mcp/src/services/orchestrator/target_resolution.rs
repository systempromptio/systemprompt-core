//! Resolves orchestration targets (all/named) to concrete server configs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::McpDomainResult;
use systemprompt_identifiers::ServiceName;

use super::McpOrchestrator;
use crate::McpServerConfig;

impl McpOrchestrator {
    pub(super) async fn list_target_servers(
        &self,
        service_name: Option<ServiceName>,
        enabled_only: bool,
    ) -> McpDomainResult<Vec<McpServerConfig>> {
        match service_name {
            Some(name) => {
                let servers = self.registry().get_managed_servers()?;
                Ok(servers
                    .into_iter()
                    .filter(|s| name.as_str() == s.name)
                    .collect())
            },
            None => {
                if enabled_only {
                    self.registry().get_managed_servers()
                } else {
                    self.database().get_running_servers().await
                }
            },
        }
    }
}
