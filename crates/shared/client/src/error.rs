use thiserror::Error;

/// `Result` alias used by every public method on
/// [`SystempromptClient`](crate::SystempromptClient).
pub type ClientResult<T> = Result<T, ClientError>;

/// Errors produced by [`SystempromptClient`](crate::SystempromptClient).
///
/// Wraps the transport-level `reqwest::Error` and JSON-level
/// `serde_json::Error` via `#[from]` so callers can use `?` directly.
/// Application-level failures (HTTP non-2xx, missing token, missing
/// resource) are represented as dedicated variants.
#[derive(Debug, Error)]
pub enum ClientError {
    /// Underlying `reqwest` transport failure (connect, TLS, decode, etc.).
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    /// Server returned a non-2xx response. `details` carries the raw response
    /// body when available.
    #[error("API error: {status} - {message}")]
    ApiError {
        /// HTTP status code.
        status: u16,
        /// Short, human-readable message extracted from the response.
        message: String,
        /// Raw response body when available.
        details: Option<String>,
    },

    /// Failed to (de)serialise a JSON payload.
    #[error("Failed to parse JSON: {0}")]
    JsonError(#[from] serde_json::Error),

    /// The client has no token configured, or the configured token was
    /// rejected by `/auth/me`.
    #[error("Authentication failed: {0}")]
    AuthError(String),

    /// Server returned 404 (or equivalent) for the requested resource.
    #[error("Resource not found: {0}")]
    NotFound(String),

    /// The request did not complete inside the configured timeout.
    #[error("Request timeout")]
    Timeout,

    /// Server is reachable but reported itself as unhealthy / unavailable.
    #[error("Server unavailable: {0}")]
    ServerUnavailable(String),

    /// The supplied configuration (base URL, timeout, etc.) is invalid.
    #[error("Invalid configuration: {0}")]
    ConfigError(String),
}

impl ClientError {
    /// Construct an [`ClientError::ApiError`] from an HTTP status and raw
    /// response body. The body is stored in both `message` and `details`.
    pub fn from_response(status: u16, body: String) -> Self {
        Self::ApiError {
            status,
            details: Some(body.clone()),
            message: body,
        }
    }

    /// Returns `true` for error classes a caller can sensibly retry (timeout,
    /// transient transport failure, server-reported unavailability).
    /// Application errors (4xx, auth, not-found, config, JSON) are
    /// non-retryable.
    pub const fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Timeout | Self::ServerUnavailable(_) | Self::HttpError(_)
        )
    }
}
