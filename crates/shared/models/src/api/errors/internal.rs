//! Internal `thiserror`-derived error type used by the application
//! tier and converted into the public [`super::ApiError`] envelope at
//! the HTTP boundary.

use super::{ApiError, ErrorCode};

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

    #[must_use]
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

#[cfg(feature = "web")]
impl axum::response::IntoResponse for InternalApiError {
    fn into_response(self) -> axum::response::Response {
        let error: ApiError = self.into();
        error.into_response()
    }
}
