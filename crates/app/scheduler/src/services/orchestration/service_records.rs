//! The two shapes the verifier reconciles: a service as the manifest declares
//! it, and a service as the `services` table last recorded it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::state_types::ServiceType;

#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub name: String,
    pub service_type: ServiceType,
    pub port: u16,
    pub enabled: bool,
}

impl ServiceConfig {
    #[must_use]
    pub fn list_from_manifest(services: &systemprompt_models::ServicesConfig) -> Vec<Self> {
        let agents = services.agents.iter().map(|(name, agent)| Self {
            name: name.clone(),
            service_type: ServiceType::Agent,
            port: agent.port,
            enabled: agent.enabled,
        });
        let mcp_servers = services
            .mcp_servers
            .iter()
            .filter(|(_, mcp)| mcp.server_type != systemprompt_models::mcp::McpServerType::External)
            .filter_map(|(name, mcp)| {
                Some(Self {
                    name: name.clone(),
                    service_type: ServiceType::Mcp,
                    port: mcp.port?,
                    enabled: mcp.enabled,
                })
            });
        agents.chain(mcp_servers).collect()
    }
}

#[derive(Debug, Clone)]
pub struct DbServiceRecord {
    pub name: String,
    pub service_type: String,
    pub status: String,
    pub pid: Option<i64>,
    pub port: i32,
    pub updated_at_epoch: Option<f64>,
}
