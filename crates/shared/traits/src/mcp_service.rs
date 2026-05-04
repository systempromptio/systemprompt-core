//! MCP service registry trait used by callers that need to resolve servers.

use std::sync::Arc;

/// Result alias for [`McpServiceProvider`] operations.
pub type McpServiceResult<T> = Result<T, McpServiceProviderError>;

/// Errors returned by MCP service providers.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum McpServiceProviderError {
    /// No registered server matches the supplied name.
    #[error("Server not found: {0}")]
    ServerNotFound(String),

    /// The registry is not currently usable.
    #[error("Registry unavailable")]
    RegistryUnavailable,

    /// Catch-all for unexpected provider failures.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for McpServiceProviderError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

/// Minimal description of a registered MCP server.
#[derive(Debug, Clone)]
pub struct McpServerMetadata {
    /// Logical server name as it appears in configuration.
    pub name: String,
    /// Endpoint clients should connect to.
    pub endpoint: String,
}

/// Resolve MCP servers and validate the registry contents.
pub trait McpServiceProvider: Send + Sync {
    /// Return the MCP protocol version this provider speaks.
    fn protocol_version(&self) -> &str;

    /// Look up a server by `name`.
    fn find_server(&self, name: &str) -> McpServiceResult<Option<McpServerMetadata>>;

    /// Run a structural sanity check over the registry.
    fn validate_registry(&self) -> McpServiceResult<()>;
}

/// Shared `Arc` alias for [`McpServiceProvider`].
pub type DynMcpServiceProvider = Arc<dyn McpServiceProvider>;
