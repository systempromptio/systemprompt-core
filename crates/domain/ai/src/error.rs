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

/// Top-level error type produced by the AI domain crate.
#[derive(Debug, Error)]
pub enum AiError {
    /// The caller did not pin a model and the provider has no default.
    #[error("Model not specified and no default available for provider {provider}")]
    ModelNotSpecified {
        /// Provider whose configuration was missing a default model.
        provider: String,
    },

    /// A required field on [`crate::models::ai::AiRequest`] metadata is absent.
    #[error("Request metadata missing required field: {field}")]
    MissingMetadata {
        /// Name of the missing metadata field.
        field: String,
    },

    /// The request had no associated [`systemprompt_models::RequestContext`].
    #[error("User context required for billing and audit trails")]
    MissingUserContext,

    /// The provider returned a syntactically valid but empty response.
    #[error("Provider {provider} returned empty response")]
    EmptyProviderResponse {
        /// The provider that returned the empty response.
        provider: String,
    },

    /// A tool's JSON schema failed validation before dispatch.
    #[error("Tool call schema validation failed: {reason}")]
    InvalidToolSchema {
        /// Human-readable reason the schema was rejected.
        reason: String,
    },

    /// The named MCP service requires authentication that has not been
    /// provided.
    #[error("Authentication required for service {service_id}")]
    AuthenticationRequired {
        /// Identifier of the service requiring authentication.
        service_id: String,
    },

    /// All retry attempts for structured-output validation were exhausted.
    #[error("Structured output validation failed after {retries} attempts: {details}")]
    StructuredOutputFailed {
        /// Number of attempts made before giving up.
        retries: usize,
        /// Detail of the final failure.
        details: String,
    },

    /// A provider call surfaced a generic failure that is not otherwise typed.
    #[error("Provider {provider} error: {message}")]
    ProviderError {
        /// Name of the upstream provider.
        provider: String,
        /// The provider's error message.
        message: String,
    },

    /// A typed [`LlmProviderError`] from
    /// [`systemprompt_provider_contracts`].
    #[error(transparent)]
    Provider(#[from] LlmProviderError),

    /// `serde_json` failed to encode or decode a payload.
    #[error("Serialization failed: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// A `reqwest` HTTP call failed.
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// An I/O error occurred while interacting with the local filesystem.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Message history could not be serialized to JSON for storage.
    #[error("Message history cannot be serialized to JSON")]
    MessageSerializationFailed,

    /// A tool definition was missing a required field.
    #[error("Tool {tool_name} missing required field: {field}")]
    MissingToolField {
        /// Name of the offending tool.
        tool_name: String,
        /// Field that was missing.
        field: String,
    },

    /// A tool definition has an empty `description`.
    #[error("Tool description cannot be empty for tool: {tool_name}")]
    EmptyToolDescription {
        /// Name of the offending tool.
        tool_name: String,
    },

    /// A response that was expected to contain tool calls had none.
    #[error("No tool calls found in provider response")]
    NoToolCalls,

    /// The provider signalled rate limiting.
    #[error("Rate limit exceeded for provider {provider}: {details}")]
    RateLimit {
        /// Provider that rate-limited the request.
        provider: String,
        /// Provider-supplied detail.
        details: String,
    },

    /// Authentication with the upstream provider was rejected.
    #[error("Invalid API credentials for provider {provider}")]
    AuthenticationFailed {
        /// Provider whose credentials were rejected.
        provider: String,
    },

    /// AI configuration was invalid or incomplete.
    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    /// A persistence operation failed.
    #[error("Database operation failed: {0}")]
    DatabaseError(String),

    /// MCP service lookup returned nothing.
    #[error("MCP service {service_id} not found or not configured")]
    McpServiceNotFound {
        /// Service identifier requested.
        service_id: String,
    },

    /// An MCP service requires an OAuth token that was not supplied.
    #[error("MCP service {service_id} requires OAuth authentication but no token available")]
    McpAuthenticationMissing {
        /// Service identifier requested.
        service_id: String,
    },

    /// Looking up service authentication requirements failed.
    #[error("Failed to determine service authentication requirements: {details}")]
    ServiceAuthCheckFailed {
        /// Detail of the lookup failure.
        details: String,
    },

    /// File / blob storage failed.
    #[error("Storage operation failed: {0}")]
    StorageError(String),

    /// User-supplied input was rejected.
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Regex compilation or matching failed.
    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    /// A tool provider call failed.
    #[error(transparent)]
    ToolProvider(#[from] systemprompt_traits::ToolProviderError),

    /// Bootstrapping the secrets store failed.
    #[error(transparent)]
    Secrets(#[from] systemprompt_config::SecretsBootstrapError),

    /// Catch-all for legacy `anyhow`-shaped failures from internal helpers.
    /// Preserves the source chain.
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

/// Persistence-layer error returned by every `*Repository` type in
/// [`crate::repository`].
#[derive(Debug, Error)]
pub enum RepositoryError {
    /// No row was found for the supplied identifier.
    #[error("AI request not found: {0}")]
    NotFound(Uuid),

    /// A `sqlx` operation failed at the driver level.
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// A field on the row failed validation when reconstructing the model.
    #[error("Invalid data: {field} - {reason}")]
    InvalidData {
        /// Column or field that failed validation.
        field: String,
        /// Reason the value was rejected.
        reason: String,
    },

    /// The underlying [`systemprompt_database::DbPool`] could not be
    /// initialized.
    #[error("Database pool initialization failed: {0}")]
    PoolInitialization(String),
}

/// Convenience alias for `Result<T, AiError>` used by every public service in
/// the [`crate::services`] module.
pub type Result<T> = std::result::Result<T, AiError>;

impl From<RepositoryError> for AiError {
    fn from(error: RepositoryError) -> Self {
        Self::DatabaseError(error.to_string())
    }
}
