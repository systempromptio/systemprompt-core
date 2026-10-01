//! Proxy error types and their HTTP status mapping.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Body;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use systemprompt_identifiers::error::IdValidationError;
use systemprompt_mcp::McpDomainError;
use systemprompt_models::api::ApiError;
use systemprompt_traits::RegistryError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProxyError {
    #[error("Service '{service}' not found in inventory")]
    ServiceNotFound { service: String },

    #[error("Service '{service}' is not running (status: {status})")]
    ServiceNotRunning { service: String, status: String },

    #[error("Failed to connect to {service} at {url}: {source}")]
    ConnectionFailed {
        service: String,
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("Request to {service} timed out")]
    Timeout { service: String },

    #[error("Invalid response from {service}")]
    InvalidResponse {
        service: String,
        #[source]
        source: ResponseBuildError,
    },

    #[error("Failed to build URL for {service}: {reason}")]
    UrlConstructionFailed { service: String, reason: String },

    #[error("Failed to read the request body")]
    BodyExtractionFailed {
        #[source]
        source: axum::Error,
    },

    #[error("Invalid HTTP method '{method}'")]
    InvalidMethod {
        method: String,
        #[source]
        source: http::method::InvalidMethod,
    },

    #[error("Database error when looking up service '{service}': {source}")]
    DatabaseError {
        service: String,
        #[source]
        source: systemprompt_traits::RepositoryError,
    },

    #[error("Authentication required for service '{service}'")]
    AuthenticationRequired { service: String },

    #[error("OAuth challenge response")]
    AuthChallenge(Box<Response<Body>>),

    #[error("Access forbidden for service '{service}'")]
    Forbidden { service: String },

    #[error("Missing request context: {message}")]
    MissingContext { message: String },

    #[error("Service name '{service}' is not a valid agent name")]
    InvalidServiceName {
        service: String,
        #[source]
        source: IdValidationError,
    },

    #[error("Service registry lookup failed for '{service}'")]
    RegistryLookupFailed {
        service: String,
        #[source]
        source: RegistryError,
    },

    #[error("Service '{service}' could not be restarted")]
    RestartFailed {
        service: String,
        #[source]
        source: McpDomainError,
    },

    #[error("Connect your account for '{service}' before using this server")]
    ProviderNotConnected { service: String },

    #[error("External MCP server '{service}' could not be resolved")]
    ExternalResolveFailed {
        service: String,
        #[source]
        source: McpDomainError,
    },

    #[error("MCP registry could not be read while resolving '{service}': {source}")]
    RegistryUnavailable {
        service: String,
        #[source]
        source: McpDomainError,
    },
}

#[derive(Debug, Error)]
pub enum ResponseBuildError {
    #[error("upstream returned an invalid HTTP status {status}")]
    Status {
        status: u16,
        #[source]
        source: http::status::InvalidStatusCode,
    },
    #[error("failed to read the upstream response body")]
    Body(#[source] reqwest::Error),
    #[error("failed to assemble the proxied response")]
    Assemble(#[source] Box<http::Error>),
}

impl ProxyError {
    pub fn to_status_code(&self) -> StatusCode {
        match self {
            Self::ServiceNotFound { .. } => StatusCode::NOT_FOUND,
            Self::ServiceNotRunning { .. } | Self::RestartFailed { .. } => {
                StatusCode::SERVICE_UNAVAILABLE
            },
            Self::ConnectionFailed { .. }
            | Self::InvalidResponse { .. }
            | Self::ExternalResolveFailed { .. } => StatusCode::BAD_GATEWAY,
            Self::Timeout { .. } => StatusCode::GATEWAY_TIMEOUT,
            Self::UrlConstructionFailed { .. }
            | Self::DatabaseError { .. }
            | Self::RegistryUnavailable { .. }
            | Self::RegistryLookupFailed { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Self::BodyExtractionFailed { .. }
            | Self::InvalidMethod { .. }
            | Self::InvalidServiceName { .. } => StatusCode::BAD_REQUEST,
            Self::AuthenticationRequired { .. } | Self::MissingContext { .. } => {
                StatusCode::UNAUTHORIZED
            },
            Self::AuthChallenge(response) => response.status(),
            Self::Forbidden { .. } => StatusCode::FORBIDDEN,
            Self::ProviderNotConnected { .. } => StatusCode::CONFLICT,
        }
    }

    pub const fn error_key(&self) -> &'static str {
        match self {
            Self::ServiceNotFound { .. } => "service_not_found",
            Self::ServiceNotRunning { .. } => "service_not_running",
            Self::RestartFailed { .. } => "restart_failed",
            Self::ConnectionFailed { .. } => "connection_failed",
            Self::Timeout { .. } => "timeout",
            Self::InvalidResponse { .. } => "invalid_response",
            Self::UrlConstructionFailed { .. } => "url_construction_failed",
            Self::BodyExtractionFailed { .. } => "body_extraction_failed",
            Self::InvalidMethod { .. } => "invalid_method",
            Self::DatabaseError { .. } => "database_error",
            Self::AuthenticationRequired { .. } => "authentication_required",
            Self::AuthChallenge(_) => "auth_challenge",
            Self::Forbidden { .. } => "forbidden",
            Self::MissingContext { .. } => "missing_context",
            Self::InvalidServiceName { .. } => "invalid_service_name",
            Self::RegistryUnavailable { .. } => "registry_unavailable",
            Self::RegistryLookupFailed { .. } => "registry_lookup_failed",
            Self::ProviderNotConnected { .. } => "provider_not_connected",
            Self::ExternalResolveFailed { .. } => "external_resolve_failed",
        }
    }
}

impl From<ProxyError> for StatusCode {
    fn from(error: ProxyError) -> Self {
        error.to_status_code()
    }
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        if let Self::AuthChallenge(response) = self {
            return *response;
        }
        let status = self.to_status_code();
        let error_key = self.error_key();
        let api_error = match status {
            StatusCode::NOT_FOUND => ApiError::not_found(self.to_string()),
            StatusCode::UNAUTHORIZED => ApiError::unauthorized(self.to_string()),
            StatusCode::FORBIDDEN => ApiError::forbidden(self.to_string()),
            StatusCode::BAD_REQUEST => ApiError::bad_request(self.to_string()),
            StatusCode::CONFLICT => ApiError::conflict(self.to_string()),
            StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::BAD_GATEWAY
            | StatusCode::GATEWAY_TIMEOUT => ApiError::service_unavailable("Proxy request failed")
                .with_details(cause_chain(&self)),
            _ => ApiError::internal_error("Proxy request failed").with_details(cause_chain(&self)),
        };
        api_error.with_error_key(error_key).into_response()
    }
}

// Why: ProxyError owns an axum response (AuthChallenge), which is not Sync, so
// it cannot ride as an ApiError source; the chain goes to the 5xx log as
// details.
fn cause_chain(error: &dyn std::error::Error) -> String {
    let mut chain = error.to_string();
    let mut next = error.source();
    while let Some(cause) = next {
        chain.push_str(": ");
        chain.push_str(&cause.to_string());
        next = cause.source();
    }
    chain
}
