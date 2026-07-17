//! Public HTTP error envelope ([`ApiError`], [`ErrorCode`],
//! [`ValidationError`], [`ErrorResponse`]) plus the internal
//! `thiserror`-derived [`InternalApiError`] used by the application
//! tier.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    NotFound,
    BadRequest,
    Unauthorized,
    Forbidden,
    InternalError,
    ValidationError,
    ConflictError,
    RateLimited,
    ServiceUnavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validation_errors: Vec<ValidationError>,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
}

impl ApiError {
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

    #[must_use]
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    #[must_use]
    pub fn with_error_key(mut self, key: impl Into<String>) -> Self {
        self.error_key = Some(key.into());
        self
    }

    #[must_use]
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    #[must_use]
    pub fn with_validation_errors(mut self, errors: Vec<ValidationError>) -> Self {
        self.validation_errors = errors;
        self
    }

    #[must_use]
    pub fn with_trace_id(mut self, id: impl Into<String>) -> Self {
        self.trace_id = Some(id.into());
        self
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::BadRequest, message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unauthorized, message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Forbidden, message)
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InternalError, message)
    }

    pub fn validation_error(message: impl Into<String>, errors: Vec<ValidationError>) -> Self {
        Self::new(ErrorCode::ValidationError, message).with_validation_errors(errors)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::ConflictError, message)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: ApiError,
    pub api_version: String,
}

#[cfg(feature = "web")]
impl ErrorCode {
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
