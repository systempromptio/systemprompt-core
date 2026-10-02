//! MCP domain error type and conversions from `sqlx` / `rmcp` failures.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::domain_error;
use systemprompt_traits::BoxedSource;

domain_error! {
    pub enum McpDomainError {
        common: [repository, io, json, validation],

        #[error("MCP server not found: {0}")]
        ServerNotFound(String),

        #[error("Connection failed to {server}: {source}")]
        ConnectionFailed {
            server: String,
            #[source]
            source: BoxedSource,
        },

        #[error("Tool execution failed: {0}")]
        ToolExecutionFailed(String),

        #[error("Tool execution failed: {context}: {source}")]
        ToolCall {
            context: String,
            #[source]
            source: BoxedSource,
        },

        #[error("Schema validation failed: {0}")]
        SchemaValidation(String),

        #[error("Registry validation failed: {0}")]
        RegistryValidation(String),

        #[error("Port unavailable: {port} - {message}")]
        PortUnavailable { port: u16, message: String },

        #[error(
            "port {port} for MCP server {service} is held by process {pid}, which this \
             installation did not spawn; another systemprompt installation or an unrelated \
             service owns it. Free the port or move this installation's MCP ports with \
             `admin config services set --port-offset`"
        )]
        PortOwnedByForeignProcess { port: u16, pid: u32, service: String },

        #[error(
            "port {port} for MCP server {service} is held by process {pid}, whose identity \
             this platform cannot verify, so it will not be signalled. Stop it by hand, or \
             move this installation's MCP ports with `admin config services set --port-offset`"
        )]
        PortHolderUnverifiable { port: u16, pid: u32, service: String },

        #[error("Configuration error: {0}")]
        Configuration(String),

        #[error("Configuration error: {context}: {source}")]
        InvalidConfiguration {
            context: String,
            #[source]
            source: BoxedSource,
        },

        #[error("Authentication required for {0}")]
        AuthRequired(String),

        #[error(
            "token issuer does not match `{expected}`; the token predates a change to \
             `security.issuer`. Re-authenticate with \
             `systemprompt admin session login --force-new`"
        )]
        TokenIssuerMismatch { expected: String },

        #[error("token rejected: {0}")]
        TokenRejected(#[from] systemprompt_security::AuthError),

        #[error("External MCP auth unavailable for {server}: {message}")]
        ExternalAuthUnavailable { server: String, message: String },

        #[error("no provider account connected for {server}")]
        ExternalAccountNotConnected { server: String },

        #[error("Manifest error: {0}")]
        Manifest(String),

        #[error("Transport error: {context}: {source}")]
        Transport {
            context: String,
            #[source]
            source: BoxedSource,
        },

        #[error("MCP server {server} timed out after {after_ms}ms")]
        Timeout { server: String, after_ms: u64 },

        #[error("Circuit breaker open for MCP server {server}; failing fast")]
        CircuitOpen { server: String },

        #[error("MCP server {server} unavailable: concurrency limit reached")]
        DependencyUnavailable { server: String },

        #[error("Failed to start {0}")]
        ServicesFailedToStart(ServiceStartFailures),

        #[error("{0}")]
        Internal(String),

        #[error("{context}: {source}")]
        Operation {
            context: String,
            #[source]
            source: BoxedSource,
        },

        #[error("Configuration: {0}")]
        Config(#[from] systemprompt_models::errors::GlobalConfigError),

        #[error("services config: {0}")]
        ServicesConfig(#[from] systemprompt_loader::ConfigLoadError),

        #[error("extension load: {0}")]
        ExtensionLoad(#[from] systemprompt_loader::ExtensionLoadError),

        #[error("MCP client initialize: {0}")]
        ClientInitialize(#[source] Box<rmcp::service::ClientInitializeError>),

        #[error("MCP service error: {0}")]
        ServiceError(#[source] Box<rmcp::ServiceError>),

        #[error("Task join error: {0}")]
        TaskJoin(#[from] tokio::task::JoinError),

        #[error("Path error: {0}")]
        Path(#[from] systemprompt_config::paths::PathError),

        #[error("Config validation: {0}")]
        ConfigValidation(#[from] systemprompt_models::errors::ConfigValidationError),
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{service} ({source})")]
pub struct ServiceStartFailure {
    pub service: String,
    #[source]
    pub source: Box<McpDomainError>,
}

impl ServiceStartFailure {
    pub fn new(service: impl Into<String>, source: McpDomainError) -> Self {
        Self {
            service: service.into(),
            source: Box::new(source),
        }
    }
}

#[derive(Debug)]
pub struct ServiceStartFailures(pub Vec<ServiceStartFailure>);

impl std::fmt::Display for ServiceStartFailures {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} MCP service(s): ", self.0.len())?;
        for (index, failure) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{failure}")?;
        }
        Ok(())
    }
}

impl From<sqlx::Error> for McpDomainError {
    fn from(err: sqlx::Error) -> Self {
        Self::Repository(systemprompt_traits::RepositoryError::from(err))
    }
}

impl From<rmcp::service::ClientInitializeError> for McpDomainError {
    fn from(e: rmcp::service::ClientInitializeError) -> Self {
        Self::ClientInitialize(Box::new(e))
    }
}

impl From<rmcp::ServiceError> for McpDomainError {
    fn from(e: rmcp::ServiceError) -> Self {
        Self::ServiceError(Box::new(e))
    }
}

impl McpDomainError {
    pub fn operation(context: impl Into<String>, source: impl Into<BoxedSource>) -> Self {
        Self::Operation {
            context: context.into(),
            source: source.into(),
        }
    }

    pub fn invalid_configuration(
        context: impl Into<String>,
        source: impl Into<BoxedSource>,
    ) -> Self {
        Self::InvalidConfiguration {
            context: context.into(),
            source: source.into(),
        }
    }

    pub fn tool_call(context: impl Into<String>, source: impl Into<BoxedSource>) -> Self {
        Self::ToolCall {
            context: context.into(),
            source: source.into(),
        }
    }

    pub fn transport(context: impl Into<String>, source: impl Into<BoxedSource>) -> Self {
        Self::Transport {
            context: context.into(),
            source: source.into(),
        }
    }

    #[must_use]
    pub const fn classify(&self) -> systemprompt_database::resilience::Outcome {
        use systemprompt_database::resilience::Outcome;
        match self {
            Self::ConnectionFailed { .. }
            | Self::Transport { .. }
            | Self::Timeout { .. }
            | Self::ServiceError(_) => Outcome::Transient { retry_after: None },
            _ => Outcome::Permanent,
        }
    }
}

pub type McpDomainResult<T> = Result<T, McpDomainError>;
