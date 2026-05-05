use systemprompt_models::domain_error;

domain_error! {
    pub enum McpDomainError {
        common: [repository, io, json, validation, anyhow],

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

        #[error("{0}")]
        Internal(String),

        #[error("Configuration: {0}")]
        Config(#[from] systemprompt_models::errors::ConfigError),

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
}

impl From<sqlx::Error> for McpDomainError {
    fn from(err: sqlx::Error) -> Self {
        Self::Repository(systemprompt_database::RepositoryError::from(err))
    }
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
