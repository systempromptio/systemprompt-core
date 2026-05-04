//! Registry provider traits for agents and MCP servers.

use async_trait::async_trait;
use std::sync::Arc;

/// Errors returned by registry providers.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RegistryError {
    /// The lookup target was not registered.
    #[error("Not found: {0}")]
    NotFound(String),

    /// The registry could not be reached.
    #[error("Registry unavailable: {0}")]
    Unavailable(String),

    /// The registry contents are misconfigured.
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Catch-all for unexpected registry failures.
    #[error("Internal error: {0}")]
    Internal(String),
}

/// OAuth requirements declared by a registered service.
#[derive(Debug, Clone)]
pub struct ServiceOAuthConfig {
    /// Whether OAuth must be presented to call the service.
    pub required: bool,
    /// Scopes the caller must hold.
    pub scopes: Vec<String>,
    /// Required audience claim for inbound JWTs.
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

/// Description of a registered agent.
#[derive(Debug, Clone)]
pub struct AgentInfo {
    /// Agent name.
    pub name: String,
    /// Bound port.
    pub port: u16,
    /// Whether the agent is active.
    pub enabled: bool,
    /// OAuth requirements for callers.
    pub oauth: ServiceOAuthConfig,
}

/// Description of a registered MCP server.
#[derive(Debug, Clone)]
pub struct McpServerInfo {
    /// Server name.
    pub name: String,
    /// Bound port.
    pub port: u16,
    /// Whether the server is active.
    pub enabled: bool,
    /// OAuth requirements for callers.
    pub oauth: ServiceOAuthConfig,
}

/// Resolve agents in the registry.
///
/// `#[async_trait]` is required because the trait is consumed as
/// `Arc<dyn AgentRegistryProvider>` via [`DynAgentRegistryProvider`].
#[async_trait]
pub trait AgentRegistryProvider: Send + Sync {
    /// Return the [`AgentInfo`] for `name`.
    async fn get_agent(&self, name: &str) -> Result<AgentInfo, RegistryError>;

    /// List every agent that is currently enabled.
    async fn list_enabled_agents(&self) -> Result<Vec<AgentInfo>, RegistryError>;

    /// Return the agent flagged as the runtime default.
    async fn get_default_agent(&self) -> Result<AgentInfo, RegistryError>;

    /// Convenience helper that reports whether `name` resolves successfully.
    async fn agent_exists(&self, name: &str) -> bool {
        self.get_agent(name).await.is_ok()
    }
}

/// Resolve MCP servers in the registry.
///
/// `#[async_trait]` is required because the trait is consumed as
/// `Arc<dyn McpRegistryProvider>` via [`DynMcpRegistryProvider`].
#[async_trait]
pub trait McpRegistryProvider: Send + Sync {
    /// Return the [`McpServerInfo`] for `name`.
    async fn get_server(&self, name: &str) -> Result<McpServerInfo, RegistryError>;

    /// List every server that is currently enabled.
    async fn list_enabled_servers(&self) -> Result<Vec<McpServerInfo>, RegistryError>;

    /// Convenience helper that reports whether `name` resolves successfully.
    async fn server_exists(&self, name: &str) -> bool {
        self.get_server(name).await.is_ok()
    }
}

/// Shared `Arc` alias for [`AgentRegistryProvider`].
pub type DynAgentRegistryProvider = Arc<dyn AgentRegistryProvider>;

/// Shared `Arc` alias for [`McpRegistryProvider`].
pub type DynMcpRegistryProvider = Arc<dyn McpRegistryProvider>;
