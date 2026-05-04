//! Typed error hierarchy for the [`systemprompt-ai`](crate) crate.
//!
//! Two error families live here:
//!
//! - [`AiError`] — the top-level public error returned by [`crate::services`].
//!   It composes provider-level failures ([`LlmProviderError`]) and
//!   repository-level failures ([`RepositoryError`]) via `#[from]`, plus common
//!   transport / parsing errors ([`reqwest::Error`], [`serde_json::Error`],
//!   [`sqlx::Error`]).
//! - [`RepositoryError`] — the persistence-layer error returned by every
//!   `*Repository` type in [`crate::repository`].
//!
//! All public service signatures use [`Result<T>`] (i.e. `Result<T, AiError>`).
//! Provider-trait signatures continue to use the boxed
//! [`systemprompt_models::errors::ProviderResult`] and bridge through
//! `AiProvider for AiService` in
//! `crate::services::core::ai_service` (the `provider_impl` submodule).

use thiserror::Error;
use uuid::Uuid;

use systemprompt_provider_contracts::LlmProviderError;

#[derive(Debug, Error)]
pub enum AiError {
    #[error("Model not specified and no default available for provider {provider}")]
    ModelNotSpecified { provider: String },

    #[error("Request metadata missing required field: {field}")]
    MissingMetadata { field: String },

    #[error("User context required for billing and audit trails")]
    MissingUserContext,

    #[error("Provider {provider} returned empty response")]
    EmptyProviderResponse { provider: String },

    #[error("Tool call schema validation failed: {reason}")]
    InvalidToolSchema { reason: String },

    #[error("Authentication required for service {service_id}")]
    AuthenticationRequired { service_id: String },

    #[error("Structured output validation failed after {retries} attempts: {details}")]
    StructuredOutputFailed { retries: usize, details: String },

    #[error("Provider {provider} error: {message}")]
    ProviderError { provider: String, message: String },

    #[error(transparent)]
    Provider(#[from] LlmProviderError),

    #[error("Serialization failed: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Message history cannot be serialized to JSON")]
    MessageSerializationFailed,

    #[error("Tool {tool_name} missing required field: {field}")]
    MissingToolField { tool_name: String, field: String },

    #[error("Tool description cannot be empty for tool: {tool_name}")]
    EmptyToolDescription { tool_name: String },

    #[error("No tool calls found in provider response")]
    NoToolCalls,

    #[error("Rate limit exceeded for provider {provider}: {details}")]
    RateLimit { provider: String, details: String },

    #[error("Invalid API credentials for provider {provider}")]
    AuthenticationFailed { provider: String },

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Database operation failed: {0}")]
    DatabaseError(String),

    #[error("MCP service {service_id} not found or not configured")]
    McpServiceNotFound { service_id: String },

    #[error("MCP service {service_id} requires OAuth authentication but no token available")]
    McpAuthenticationMissing { service_id: String },

    #[error("Failed to determine service authentication requirements: {details}")]
    ServiceAuthCheckFailed { details: String },

    #[error("Storage operation failed: {0}")]
    StorageError(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error(transparent)]
    ToolProvider(#[from] systemprompt_traits::ToolProviderError),

    #[error(transparent)]
    Secrets(#[from] systemprompt_config::SecretsBootstrapError),

    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("AI request not found: {0}")]
    NotFound(Uuid),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Invalid data: {field} - {reason}")]
    InvalidData { field: String, reason: String },

    #[error("Database pool initialization failed: {0}")]
    PoolInitialization(String),
}

pub type Result<T> = std::result::Result<T, AiError>;

impl From<RepositoryError> for AiError {
    fn from(error: RepositoryError) -> Self {
        Self::DatabaseError(error.to_string())
    }
}
