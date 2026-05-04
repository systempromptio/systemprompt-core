use axum::body::Body;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use systemprompt_models::api::{ApiError, ErrorCode};
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

    #[error("Invalid response from {service}: {reason}")]
    InvalidResponse { service: String, reason: String },

    #[error("Failed to build URL for {service}: {reason}")]
    UrlConstructionFailed { service: String, reason: String },

    #[error("Failed to extract request body: {source}")]
    BodyExtractionFailed {
        #[source]
        source: axum::Error,
    },

    #[error("Invalid HTTP method: {reason}")]
    InvalidMethod { reason: String },

    #[error("Database error when looking up service '{service}': {source}")]
    DatabaseError {
        service: String,
        #[source]
        source: anyhow::Error,
    },

    #[error("Authentication required for service '{service}'")]
    AuthenticationRequired { service: String },

    #[error("OAuth challenge response")]
    AuthChallenge(Box<Response<Body>>),

    #[error("Access forbidden for service '{service}'")]
    Forbidden { service: String },

    #[error("Missing request context: {message}")]
    MissingContext { message: String },
}

impl ProxyError {
    pub fn to_status_code(&self) -> StatusCode {
        match self {
            Self::ServiceNotFound { .. } => StatusCode::NOT_FOUND,
            Self::ServiceNotRunning { .. } => StatusCode::SERVICE_UNAVAILABLE,
            Self::ConnectionFailed { .. } | Self::InvalidResponse { .. } => StatusCode::BAD_GATEWAY,
            Self::Timeout { .. } => StatusCode::GATEWAY_TIMEOUT,
            Self::UrlConstructionFailed { .. } | Self::DatabaseError { .. } => {
                StatusCode::INTERNAL_SERVER_ERROR
            },
            Self::BodyExtractionFailed { .. } | Self::InvalidMethod { .. } => {
                StatusCode::BAD_REQUEST
            },
            Self::AuthenticationRequired { .. } | Self::MissingContext { .. } => {
                StatusCode::UNAUTHORIZED
            },
            Self::AuthChallenge(response) => response.status(),
            Self::Forbidden { .. } => StatusCode::FORBIDDEN,
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
        match self {
            Self::AuthChallenge(response) => (*response).into_response(),
            ref error => {
                let status = error.to_status_code();
                let error_type = match &self {
                    Self::ServiceNotFound { .. } => "service_not_found",
                    Self::ServiceNotRunning { .. } => "service_not_running",
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
                };

                if status.is_server_error() {
                    tracing::error!(
                        error_type = %error_type,
                        status_code = %status.as_u16(),
                        error = %self,
                        "Proxy server error"
                    );
                } else if status.is_client_error() {
                    tracing::warn!(
                        error_type = %error_type,
                        status_code = %status.as_u16(),
                        error = %self,
                        "Proxy client error"
                    );
                }

                let message = self.to_string();
                let api_error = match status {
                    StatusCode::NOT_FOUND => ApiError::not_found(message),
                    StatusCode::UNAUTHORIZED => ApiError::unauthorized(message),
                    StatusCode::FORBIDDEN => ApiError::forbidden(message),
                    StatusCode::BAD_REQUEST => ApiError::bad_request(message),
                    StatusCode::SERVICE_UNAVAILABLE
                    | StatusCode::BAD_GATEWAY
                    | StatusCode::GATEWAY_TIMEOUT => {
                        ApiError::new(ErrorCode::ServiceUnavailable, message)
                    },
                    _ => ApiError::internal_error(message),
                };
                api_error.into_response()
            },
        }
    }
}
