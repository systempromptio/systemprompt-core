use thiserror::Error;

#[derive(Error, Debug)]
pub enum McpDomainError {
    #[error("MCP server not found: {0}")]
    ServerNotFound(String),

    #[error("Connection failed to {server}: {message}")]
    ConnectionFailed { server: String, message: String },

    #[error("Tool execution failed: {0}")]
    ToolExecutionFailed(String),

    #[error("Schema validation failed: {0}")]
    SchemaValidation(String),

    #[error("Registry validation failed: {0}")]
    RegistryValidation(String),

    #[error("Process spawn failed for {server}: {message}")]
    ProcessSpawn { server: String, message: String },

    #[error("Port unavailable: {port} - {message}")]
    PortUnavailable { port: u16, message: String },

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Authentication required for {0}")]
    AuthRequired(String),

    #[error("Manifest error: {0}")]
    Manifest(String),

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Repository error: {0}")]
    Repository(#[from] systemprompt_database::RepositoryError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("{0}")]
    Internal(String),

    #[error("Configuration: {0}")]
    Config(#[from] systemprompt_models::errors::ConfigError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),

    #[error("services config: {0}")]
    ServicesConfig(#[from] systemprompt_loader::ConfigLoadError),

    #[error("extension load: {0}")]
    ExtensionLoad(#[from] systemprompt_loader::ExtensionLoadError),

    #[error("MCP client initialize: {0}")]
    ClientInitialize(String),

    #[error("MCP service error: {0}")]
    ServiceError(String),

    #[error("Task join error: {0}")]
    TaskJoin(#[from] tokio::task::JoinError),

    #[error("Path error: {0}")]
    Path(String),

    #[error("Config validation: {0}")]
    ConfigValidation(String),
}

impl From<rmcp::service::ClientInitializeError> for McpDomainError {
    fn from(e: rmcp::service::ClientInitializeError) -> Self {
        Self::ClientInitialize(e.to_string())
    }
}

impl From<rmcp::ServiceError> for McpDomainError {
    fn from(e: rmcp::ServiceError) -> Self {
        Self::ServiceError(e.to_string())
    }
}

impl From<systemprompt_models::errors::ConfigValidationError> for McpDomainError {
    fn from(e: systemprompt_models::errors::ConfigValidationError) -> Self {
        Self::ConfigValidation(e.to_string())
    }
}

impl From<systemprompt_models::paths::PathError> for McpDomainError {
    fn from(e: systemprompt_models::paths::PathError) -> Self {
        Self::Path(e.to_string())
    }
}

pub type McpDomainResult<T> = Result<T, McpDomainError>;
