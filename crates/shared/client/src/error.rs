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

    #[error("Failed to parse JSON: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Authentication failed: {0}")]
    AuthError(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Request timeout")]
    Timeout,

    #[error("Server unavailable: {0}")]
    ServerUnavailable(String),

    #[error("Invalid configuration: {0}")]
    ConfigError(String),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

impl ClientError {
    pub fn from_response(status: u16, body: String) -> Self {
        Self::ApiError {
            status,
            details: Some(body.clone()),
            message: body,
        }
    }

    pub const fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Timeout | Self::ServerUnavailable(_) | Self::HttpError(_)
        )
    }
}
