//! Implementation of AgentRegistryProvider trait for the agent module.

use async_trait::async_trait;
use systemprompt_traits::{AgentInfo, AgentRegistryProvider, RegistryError, ServiceOAuthConfig};

use super::registry::AgentRegistry;

#[derive(Debug, Clone)]
pub struct AgentRegistryProviderService {
    registry: AgentRegistry,
}

impl AgentRegistryProviderService {
    pub async fn new() -> Result<Self, RegistryError> {
        let registry = AgentRegistry::new()
            .await
            .map_err(|e| RegistryError::Unavailable(e.to_string()))?;

        Ok(Self { registry })
    }

    pub fn from_registry(registry: AgentRegistry) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl AgentRegistryProvider for AgentRegistryProviderService {
    async fn get_agent(&self, name: &str) -> Result<AgentInfo, RegistryError> {
        let agent = self
            .registry
            .get_agent(name)
            .await
            .map_err(|e| RegistryError::NotFound(e.to_string()))?;

        Ok(AgentInfo {
            name: agent.name,
            port: agent.port,
            enabled: agent.enabled,
            oauth: ServiceOAuthConfig {
                required: agent.oauth.required,
                scopes: agent.oauth.scopes.iter().map(|s| s.to_string()).collect(),
                audience: agent.oauth.audience.to_string(),
            },
        })
    }

    async fn list_enabled_agents(&self) -> Result<Vec<AgentInfo>, RegistryError> {
        let agents = self
            .registry
            .list_enabled_agents()
            .await
            .map_err(|e| RegistryError::Unavailable(e.to_string()))?;

        Ok(agents
            .into_iter()
            .map(|agent| AgentInfo {
                name: agent.name,
                port: agent.port,
                enabled: agent.enabled,
                oauth: ServiceOAuthConfig {
                    required: agent.oauth.required,
                    scopes: agent.oauth.scopes.iter().map(|s| s.to_string()).collect(),
                    audience: agent.oauth.audience.to_string(),
                },
            })
            .collect())
    }

    async fn get_default_agent(&self) -> Result<AgentInfo, RegistryError> {
        let agent = self
            .registry
            .get_default_agent()
            .await
            .map_err(|e| RegistryError::NotFound(e.to_string()))?;

        Ok(AgentInfo {
            name: agent.name,
            port: agent.port,
            enabled: agent.enabled,
            oauth: ServiceOAuthConfig {
                required: agent.oauth.required,
                scopes: agent.oauth.scopes.iter().map(|s| s.to_string()).collect(),
                audience: agent.oauth.audience.to_string(),
            },
        })
    }
}
