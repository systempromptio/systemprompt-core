//! Typed error hierarchy for the [`systemprompt-ai`](crate) crate.
//!
//! [`AiError`] is the top-level public error returned by [`crate::services`].
//! It composes the canonical repository error ([`RepositoryError`]) returned
//! by every `*Repository` type in [`crate::repository`] via `#[from]`, plus
//! common transport / parsing errors ([`reqwest::Error`],
//! [`serde_json::Error`]).
//!
//! All public service signatures use [`Result<T>`] (i.e. `Result<T, AiError>`).
//! The dyn `AiProvider` seam returns
//! [`AiInferenceError`](systemprompt_models::errors::AiInferenceError); the
//! `From<AiError>` impl below is the single mapping onto it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use thiserror::Error;

use systemprompt_database::resilience::Outcome;
use systemprompt_identifiers::{McpServerId, McpToolName};
use systemprompt_models::wire::error::WireStreamError;
use systemprompt_traits::{AiProviderError, FileStorageError, RepositoryError};

use crate::services::storage::StorageConfigError;

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
    AuthenticationRequired { service_id: McpServerId },

    #[error("Structured output validation failed after {retries} attempts: {details}")]
    StructuredOutputFailed { retries: usize, details: String },

    #[error("Provider {provider} error: {message}")]
    ProviderError { provider: String, message: String },

    #[error("No configured provider supports model {model}")]
    NoProviderForModel { model: String },

    #[error("Upstream provider could not be resolved: {0}")]
    Upstream(#[from] crate::services::upstream::UpstreamTargetError),

    #[error("Serialization failed: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Message history cannot be serialized to JSON")]
    MessageSerializationFailed,

    #[error("Tool {tool_name} missing required field: {field}")]
    MissingToolField {
        tool_name: McpToolName,
        field: String,
    },

    #[error("Tool description cannot be empty for tool: {tool_name}")]
    EmptyToolDescription { tool_name: McpToolName },

    #[error("No tool calls found in provider response")]
    NoToolCalls,

    #[error("Rate limit exceeded for provider {provider}: {details}")]
    RateLimit { provider: String, details: String },

    #[error("Provider {provider} returned HTTP {status}: {body}")]
    HttpStatus {
        provider: String,
        status: u16,
        retry_after: Option<Duration>,
        body: String,
    },

    #[error("Provider {provider} request timed out after {after_ms}ms")]
    Timeout { provider: String, after_ms: u64 },

    #[error("Circuit breaker open for provider {provider}; failing fast")]
    CircuitOpen { provider: String },

    #[error("Provider {provider} unavailable: concurrency limit reached")]
    DependencyUnavailable { provider: String },

    #[error("Invalid API credentials for provider {provider}")]
    AuthenticationFailed { provider: String },

    #[error("Configuration error: {message}")]
    ConfigurationError { message: String },

    #[error(transparent)]
    Repository(#[from] RepositoryError),

    #[error("file persistence failed: {0}")]
    FilePersistence(#[from] AiProviderError),

    #[error("MCP service {service_id} not found or not configured")]
    McpServiceNotFound { service_id: McpServerId },

    #[error("MCP service {service_id} requires OAuth authentication but no token available")]
    McpAuthenticationMissing { service_id: McpServerId },

    #[error("Failed to determine service authentication requirements: {details}")]
    ServiceAuthCheckFailed { details: String },

    #[error("{context}: {source}")]
    Storage {
        context: String,
        #[source]
        source: FileStorageError,
    },

    #[error("invalid image storage configuration: {0}")]
    StorageConfig(#[from] StorageConfigError),

    #[error("image size {size} bytes exceeds the maximum allowed size {max} bytes")]
    ImageTooLarge { size: usize, max: usize },

    #[error("failed to decode base64 image: {0}")]
    ImageDecode(#[from] base64::DecodeError),

    #[error("provider stream failed: {0}")]
    Stream(#[from] WireStreamError),

    #[error("invalid generated file id: {0}")]
    InvalidFileId(#[source] uuid::Error),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error(transparent)]
    ToolProvider(#[from] systemprompt_traits::ToolProviderError),

    #[error("tool discovery incomplete: {0}")]
    ToolDiscovery(String),

    #[error("provider {provider} has no pricing for model {model}; refusing to bill it")]
    UnknownModel { provider: String, model: String },

    #[error(transparent)]
    Secrets(#[from] systemprompt_config::SecretsBootstrapError),

    #[error(transparent)]
    WireParse(#[from] systemprompt_models::wire::error::WireParseError),

    #[error("internal: {0}")]
    Internal(String),
}

impl From<AiError> for systemprompt_models::errors::AiInferenceError {
    fn from(err: AiError) -> Self {
        match err {
            AiError::ModelNotSpecified { ref provider }
            | AiError::EmptyProviderResponse { ref provider } => Self::Provider {
                provider: provider.clone(),
                message: err.to_string(),
            },
            AiError::ProviderError { provider, message } => Self::Provider { provider, message },
            AiError::NoProviderForModel { model } => Self::NoProviderForModel { model },
            AiError::RateLimit { provider, details } => Self::RateLimited { provider, details },
            AiError::AuthenticationFailed { provider } => Self::AuthenticationFailed { provider },
            AiError::HttpStatus { ref provider, .. }
            | AiError::Timeout { ref provider, .. }
            | AiError::CircuitOpen { ref provider }
            | AiError::DependencyUnavailable { ref provider } => Self::Unavailable {
                provider: provider.clone(),
                message: err.to_string(),
            },
            AiError::MissingMetadata { .. }
            | AiError::MissingUserContext
            | AiError::InvalidToolSchema { .. }
            | AiError::StructuredOutputFailed { .. }
            | AiError::MessageSerializationFailed
            | AiError::MissingToolField { .. }
            | AiError::EmptyToolDescription { .. }
            | AiError::InvalidInput(_)
            | AiError::ImageDecode(_)
            | AiError::InvalidFileId(_)
            | AiError::WireParse(_) => Self::InvalidRequest(Box::new(err)),
            AiError::NoToolCalls
            | AiError::McpServiceNotFound { .. }
            | AiError::McpAuthenticationMissing { .. }
            | AiError::ServiceAuthCheckFailed { .. }
            | AiError::ToolDiscovery(_)
            | AiError::ToolProvider(_) => Self::Tool(Box::new(err)),
            AiError::UnknownModel { .. }
            | AiError::AuthenticationRequired { .. }
            | AiError::ConfigurationError { .. }
            | AiError::Secrets(_)
            | AiError::StorageConfig(_)
            | AiError::Upstream(_) => Self::Configuration(Box::new(err)),
            AiError::Repository(_)
            | AiError::FilePersistence(_)
            | AiError::Storage { .. }
            | AiError::ImageTooLarge { .. } => Self::Storage(Box::new(err)),
            AiError::Stream(_)
            | AiError::SerializationError(_)
            | AiError::Http(_)
            | AiError::Io(_)
            | AiError::Regex(_)
            | AiError::Internal(_) => Self::Internal(Box::new(err)),
        }
    }
}

impl AiError {
    pub async fn from_error_response(provider: &str, response: reqwest::Response) -> Self {
        let status = response.status().as_u16();
        let retry_after = parse_retry_after(response.headers());
        let body = response
            .text()
            .await
            .unwrap_or_else(|e| format!("<unreadable body: {e}>"));
        Self::HttpStatus {
            provider: provider.to_owned(),
            status,
            retry_after,
            body,
        }
    }

    #[must_use]
    pub fn classify(&self) -> Outcome {
        match self {
            Self::HttpStatus {
                status,
                retry_after,
                ..
            } => {
                if matches!(*status, 408 | 425 | 429 | 500 | 502 | 503 | 504) {
                    Outcome::Transient {
                        retry_after: *retry_after,
                    }
                } else {
                    Outcome::Permanent
                }
            },
            Self::RateLimit { .. } | Self::Timeout { .. } => {
                Outcome::Transient { retry_after: None }
            },
            Self::Http(err) if err.is_timeout() || err.is_connect() => {
                Outcome::Transient { retry_after: None }
            },
            _ => Outcome::Permanent,
        }
    }
}

fn parse_retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    headers
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
        .map(Duration::from_secs)
}

pub type Result<T> = std::result::Result<T, AiError>;
