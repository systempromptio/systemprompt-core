//! Public HTTP error envelope ([`ApiError`], [`ErrorCode`],
//! [`ValidationError`], [`ErrorResponse`]) plus the internal
//! `thiserror`-derived [`InternalApiError`] used by the application
//! tier.

mod internal;

pub use internal::InternalApiError;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(feature = "web")]
use axum::Json;
#[cfg(feature = "web")]
use axum::http::{StatusCode, header};
#[cfg(feature = "web")]
use axum::response::IntoResponse;

/// Coarse-grained error category that drives the HTTP status code.
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Resource not found (404).
    NotFound,
    /// Malformed request (400).
    BadRequest,
    /// Unauthenticated (401).
    Unauthorized,
    /// Authenticated but forbidden (403).
    Forbidden,
    /// Catch-all server error (500).
    InternalError,
    /// Field validation failed (422).
    ValidationError,
    /// Conflict with existing state (409).
    ConflictError,
    /// Rate limit exceeded (429).
    RateLimited,
    /// Downstream service unavailable (503).
    ServiceUnavailable,
}

/// Field-level validation error attached to an [`ApiError`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// JSON pointer or dotted field path that failed validation.
    pub field: String,
    /// Human-readable validation message.
    pub message: String,
    /// Machine-readable error code.
    pub code: String,
    /// Optional structured context (constraints, limits, …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,
}

/// Public HTTP error envelope returned by the API.
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiError {
    /// Coarse-grained error category.
    pub code: ErrorCode,
    /// Human-readable summary.
    pub message: String,
    /// Optional verbose detail string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    /// Optional stable error key for client-side i18n.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_key: Option<String>,
    /// Optional URL path the error refers to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Field-level validation errors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validation_errors: Vec<ValidationError>,
    /// Wall-clock timestamp the error was assembled.
    pub timestamp: DateTime<Utc>,
    /// Optional trace identifier for log correlation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
}

impl ApiError {
    /// Build a new [`ApiError`] with the given code and message.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
            error_key: None,
            path: None,
            validation_errors: Vec::new(),
            timestamp: Utc::now(),
            trace_id: None,
        }
    }

    /// Attach a verbose detail string.
    #[must_use]
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    /// Attach a stable error key.
    #[must_use]
    pub fn with_error_key(mut self, key: impl Into<String>) -> Self {
        self.error_key = Some(key.into());
        self
    }

    /// Attach the request path.
    #[must_use]
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Replace the field-level validation errors.
    #[must_use]
    pub fn with_validation_errors(mut self, errors: Vec<ValidationError>) -> Self {
        self.validation_errors = errors;
        self
    }

    /// Attach a trace identifier.
    #[must_use]
    pub fn with_trace_id(mut self, id: impl Into<String>) -> Self {
        self.trace_id = Some(id.into());
        self
    }

    /// Convenience constructor for `404 Not Found`.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    /// Convenience constructor for `400 Bad Request`.
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::BadRequest, message)
    }

    /// Convenience constructor for `401 Unauthorized`.
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unauthorized, message)
    }

    /// Convenience constructor for `403 Forbidden`.
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Forbidden, message)
    }

    /// Convenience constructor for `500 Internal Server Error`.
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InternalError, message)
    }

    /// Convenience constructor for `422 Unprocessable Entity` carrying
    /// a field-level error list.
    pub fn validation_error(message: impl Into<String>, errors: Vec<ValidationError>) -> Self {
        Self::new(ErrorCode::ValidationError, message).with_validation_errors(errors)
    }

    /// Convenience constructor for `409 Conflict`.
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::ConflictError, message)
    }
}

/// Top-level wire shape that wraps an [`ApiError`] alongside the API
/// version string.
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// The error payload.
    pub error: ApiError,
    /// API version string.
    pub api_version: String,
}

#[cfg(feature = "web")]
impl ErrorCode {
    /// Map this category to the HTTP status code that should surface
    /// to clients.
    #[must_use]
    pub const fn status_code(&self) -> StatusCode {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::BadRequest => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::ValidationError => StatusCode::UNPROCESSABLE_ENTITY,
            Self::ConflictError => StatusCode::CONFLICT,
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Self::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[cfg(feature = "web")]
impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = self.code.status_code();

        if status.is_server_error() {
            tracing::error!(
                error_code = ?self.code,
                message = %self.message,
                path = ?self.path,
                trace_id = ?self.trace_id,
                "API server error response"
            );
        } else if status.is_client_error() {
            tracing::warn!(
                error_code = ?self.code,
                message = %self.message,
                path = ?self.path,
                trace_id = ?self.trace_id,
                "API client error response"
            );
        }

        let mut response = (status, Json(self)).into_response();

        if status == StatusCode::UNAUTHORIZED
            && let Ok(header_value) =
                "Bearer resource_metadata=\"/.well-known/oauth-protected-resource\"".parse()
        {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, header_value);
        }

        response
    }
}
