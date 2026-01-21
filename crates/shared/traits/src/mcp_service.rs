use std::sync::Arc;

pub type McpServiceResult<T> = Result<T, McpServiceProviderError>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum McpServiceProviderError {
    #[error("Server not found: {0}")]
    ServerNotFound(String),

    #[error("Registry unavailable")]
    RegistryUnavailable,

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for McpServiceProviderError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct McpServerMetadata {
    pub name: String,
    pub endpoint: String,
}

pub trait McpServiceProvider: Send + Sync {
    fn protocol_version(&self) -> &str;

    fn find_server(&self, name: &str) -> McpServiceResult<Option<McpServerMetadata>>;

    fn validate_registry(&self) -> McpServiceResult<()>;
}

pub type DynMcpServiceProvider = Arc<dyn McpServiceProvider>;
