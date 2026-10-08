//! `ClientError` enum and `ClientResult` alias for HTTP API client failures.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

pub type ClientResult<T> = Result<T, ClientError>;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("API error: {status} - {message}")]
    ApiError {
        status: u16,
        message: String,
        details: Option<String>,
    },

    #[error("API error: {status} - response body unreadable")]
    UnreadableErrorBody {
        status: u16,
        #[source]
        source: reqwest::Error,
    },

    #[error("Failed to parse JSON: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Authentication failed: {message}")]
    AuthError { message: String },

    #[error("Request timeout")]
    Timeout,

    #[error("Server unavailable: {0}")]
    ServerUnavailable(String),

    #[error("Server unavailable: undecodable cli event from server: {0}")]
    UndecodableEvent(#[source] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl ClientError {
    pub const fn from_response(status: u16, body: String) -> Self {
        Self::ApiError {
            status,
            message: body,
            details: None,
        }
    }

    pub const fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Timeout
                | Self::ServerUnavailable(_)
                | Self::UndecodableEvent(_)
                | Self::HttpError(_)
        )
    }
}
