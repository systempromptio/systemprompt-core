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
}

pub type McpDomainResult<T> = Result<T, McpDomainError>;
