//! Registry provider traits for agents and MCP servers.

use async_trait::async_trait;
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Registry unavailable: {0}")]
    Unavailable(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Clone)]
pub struct ServiceOAuthConfig {
    pub required: bool,
    pub scopes: Vec<String>,
    pub audience: String,
}

impl Default for ServiceOAuthConfig {
    fn default() -> Self {
        Self {
            required: true,
            scopes: Vec::new(),
            audience: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub name: String,
    pub port: u16,
    pub enabled: bool,
    pub oauth: ServiceOAuthConfig,
}

#[derive(Debug, Clone)]
pub struct McpServerInfo {
    pub name: String,
    pub port: u16,
    pub enabled: bool,
    pub oauth: ServiceOAuthConfig,
}

#[async_trait]
pub trait AgentRegistryProvider: Send + Sync {
    async fn get_agent(&self, name: &str) -> Result<AgentInfo, RegistryError>;

    async fn list_enabled_agents(&self) -> Result<Vec<AgentInfo>, RegistryError>;

    async fn get_default_agent(&self) -> Result<AgentInfo, RegistryError>;

    async fn agent_exists(&self, name: &str) -> bool {
        self.get_agent(name).await.is_ok()
    }
}

#[async_trait]
pub trait McpRegistryProvider: Send + Sync {
    async fn get_server(&self, name: &str) -> Result<McpServerInfo, RegistryError>;

    async fn list_enabled_servers(&self) -> Result<Vec<McpServerInfo>, RegistryError>;

    async fn server_exists(&self, name: &str) -> bool {
        self.get_server(name).await.is_ok()
    }
}

pub type DynAgentRegistryProvider = Arc<dyn AgentRegistryProvider>;

pub type DynMcpRegistryProvider = Arc<dyn McpRegistryProvider>;
