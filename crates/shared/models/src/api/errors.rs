use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(feature = "web")]
use axum::http::StatusCode;
#[cfg(feature = "web")]
use axum::response::IntoResponse;
#[cfg(feature = "web")]
use axum::Json;

#[derive(Debug, thiserror::Error)]
pub enum InternalApiError {
    #[error("Resource not found: {resource_type} with ID '{id}'")]
    NotFound { resource_type: String, id: String },

    #[error("Bad request: {message}")]
    BadRequest { message: String },

    #[error("Unauthorized access: {reason}")]
    Unauthorized { reason: String },

    #[error("Access forbidden: {resource} - {reason}")]
    Forbidden { resource: String, reason: String },

    #[error("Validation failed for field '{field}': {reason}")]
    ValidationError { field: String, reason: String },

    #[error("Conflict: {resource} already exists")]
    ConflictError { resource: String },

    #[error("Rate limit exceeded for {resource}")]
    RateLimited { resource: String },

    #[error("Service temporarily unavailable: {service}")]
    ServiceUnavailable { service: String },

    #[error("Database operation failed: {message}")]
    DatabaseError { message: String },

    #[error("JSON serialization failed")]
    JsonError(#[from] serde_json::Error),

    #[error("Authentication token error: {message}")]
    AuthenticationError { message: String },

    #[error("Internal server error: {message}")]
    InternalError { message: String },
}

impl InternalApiError {
    pub fn not_found(resource_type: impl Into<String>, id: impl Into<String>) -> Self {
        Self::NotFound {
            resource_type: resource_type.into(),
            id: id.into(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest {
            message: message.into(),
        }
    }

    pub fn unauthorized(reason: impl Into<String>) -> Self {
        Self::Unauthorized {
            reason: reason.into(),
        }
    }

    pub fn forbidden(resource: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Forbidden {
            resource: resource.into(),
            reason: reason.into(),
        }
    }

    pub fn validation_error(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::ValidationError {
            field: field.into(),
            reason: reason.into(),
        }
    }

    pub fn conflict(resource: impl Into<String>) -> Self {
        Self::ConflictError {
            resource: resource.into(),
        }
    }

    pub fn rate_limited(resource: impl Into<String>) -> Self {
        Self::RateLimited {
            resource: resource.into(),
        }
    }

    pub fn service_unavailable(service: impl Into<String>) -> Self {
        Self::ServiceUnavailable {
            service: service.into(),
        }
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::InternalError {
            message: message.into(),
        }
    }

    pub fn database_error(message: impl Into<String>) -> Self {
        Self::DatabaseError {
            message: message.into(),
        }
    }

    pub fn authentication_error(message: impl Into<String>) -> Self {
        Self::AuthenticationError {
            message: message.into(),
        }
    }

    pub const fn error_code(&self) -> ErrorCode {
        match self {
            Self::NotFound { .. } => ErrorCode::NotFound,
            Self::BadRequest { .. } => ErrorCode::BadRequest,
            Self::Unauthorized { .. } => ErrorCode::Unauthorized,
            Self::Forbidden { .. } => ErrorCode::Forbidden,
            Self::ValidationError { .. } => ErrorCode::ValidationError,
            Self::ConflictError { .. } => ErrorCode::ConflictError,
            Self::RateLimited { .. } => ErrorCode::RateLimited,
            Self::ServiceUnavailable { .. } => ErrorCode::ServiceUnavailable,
            Self::DatabaseError { .. }
            | Self::JsonError(_)
            | Self::AuthenticationError { .. }
            | Self::InternalError { .. } => ErrorCode::InternalError,
        }
    }
}

impl From<InternalApiError> for ApiError {
    fn from(error: InternalApiError) -> Self {
        let code = error.error_code();
        let message = error.to_string();
        let details = match &error {
            InternalApiError::NotFound { resource_type, id } => Some(format!(
                "The requested {resource_type} with ID '{id}' does not exist"
            )),
            InternalApiError::ValidationError { field, reason } => {
                Some(format!("Field '{field}': {reason}"))
            },
            InternalApiError::Forbidden { resource, reason } => {
                Some(format!("Access to {resource} denied: {reason}"))
            },
            InternalApiError::DatabaseError { message } => {
                Some(format!("Database error: {message}"))
            },
            InternalApiError::JsonError(e) => Some(format!("JSON processing error: {e}")),
            InternalApiError::AuthenticationError { message } => {
                Some(format!("Authentication error: {message}"))
            },
            InternalApiError::BadRequest { .. }
            | InternalApiError::Unauthorized { .. }
            | InternalApiError::ConflictError { .. }
            | InternalApiError::RateLimited { .. }
            | InternalApiError::ServiceUnavailable { .. }
            | InternalApiError::InternalError { .. } => None,
        };

        let api_error = Self::new(code, message);
        if let Some(d) = details {
            api_error.with_details(d)
        } else {
            api_error
        }
    }
}

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

    pub context: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiError {
    pub code: ErrorCode,

    pub message: String,

    pub details: Option<String>,

    pub error_key: Option<String>,

    pub path: Option<String>,

    #[serde(default)]
    pub validation_errors: Vec<ValidationError>,

    pub timestamp: DateTime<Utc>,

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

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    pub fn with_error_key(mut self, key: impl Into<String>) -> Self {
        self.error_key = Some(key.into());
        self
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn with_validation_errors(mut self, errors: Vec<ValidationError>) -> Self {
        self.validation_errors = errors;
        self
    }

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

        (status, Json(self)).into_response()
    }
}

#[cfg(feature = "web")]
impl IntoResponse for InternalApiError {
    fn into_response(self) -> axum::response::Response {
        let error: ApiError = self.into();
        error.into_response()
    }
}
