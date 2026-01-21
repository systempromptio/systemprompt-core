use thiserror::Error;

#[derive(Error, Debug)]
pub enum McpError {
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

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("{0}")]
    Internal(String),
}

pub type McpResult<T> = Result<T, McpError>;

impl From<anyhow::Error> for McpError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}
