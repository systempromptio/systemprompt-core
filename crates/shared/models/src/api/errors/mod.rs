//! The single HTTP error model: [`ApiError`] with its [`ErrorCode`] status
//! class, plus [`ValidationError`] field detail.
//!
//! An [`ApiError`] carries a stable machine code, a public message and,
//! optionally, the internal cause as a source that is logged and never
//! serialised. The wire shape enforces the redaction rule itself: a 5xx
//! serialises the fixed public message of its code with no details or
//! validation errors, whatever text the error was built with, so internal
//! error text cannot reach a response body. Repository errors convert through
//! the one canonical `From<RepositoryError>` mapping in the `repository`
//! module; an identifier that fails to parse converts through
//! `From<IdValidationError>` (400) in the `identifier` module.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod extension;
mod identifier;
mod repository;
#[cfg(feature = "web")]
mod response;
mod wire;

use std::error::Error;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use systemprompt_identifiers::TraceId;
use systemprompt_traits::BoxedSource;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl ErrorCode {
    #[must_use]
    pub const fn is_server_error(self) -> bool {
        matches!(self, Self::InternalError | Self::ServiceUnavailable)
    }

    #[must_use]
    pub const fn public_server_message(self) -> &'static str {
        match self {
            Self::ServiceUnavailable => "Service temporarily unavailable",
            _ => "Internal server error",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    // JSON: Validation context echoes the offending request fragment, whatever its shape.
    pub context: Option<Value>,
}

/// The HTTP error envelope every non-protocol route answers with.
#[derive(Debug, Deserialize)]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(default)]
    pub details: Option<String>,
    #[serde(default)]
    pub error_key: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub validation_errors: Vec<ValidationError>,
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub trace_id: Option<TraceId>,
    #[serde(skip)]
    source: Option<BoxedSource>,
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
            source: None,
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
    pub fn with_trace_id(mut self, id: TraceId) -> Self {
        self.trace_id = Some(id);
        self
    }

    #[must_use]
    pub fn with_source(mut self, source: impl Into<BoxedSource>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn source(&self) -> Option<&(dyn Error + Send + Sync + 'static)> {
        self.source.as_deref()
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

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::ConflictError, message)
    }

    pub fn rate_limited(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::RateLimited, message)
    }

    pub fn validation_error(message: impl Into<String>, errors: Vec<ValidationError>) -> Self {
        Self::new(ErrorCode::ValidationError, message).with_validation_errors(errors)
    }

    pub fn internal_error(context: &'static str) -> Self {
        Self::new(ErrorCode::InternalError, context)
    }

    pub fn internal(context: &'static str, source: impl Into<BoxedSource>) -> Self {
        Self::internal_error(context).with_source(source)
    }

    pub fn service_unavailable(context: &'static str) -> Self {
        Self::new(ErrorCode::ServiceUnavailable, context)
    }
}
