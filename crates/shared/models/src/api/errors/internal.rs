//! Internal `thiserror`-derived error type used by the application
//! tier and converted into the public [`super::ApiError`] envelope at
//! the HTTP boundary.

use super::{ApiError, ErrorCode};

/// Application-tier error type. Internal callers `?` this and the
/// boundary handler converts it into [`ApiError`] for the wire
/// response.
#[derive(Debug, thiserror::Error)]
pub enum InternalApiError {
    /// Resource lookup failed.
    #[error("Resource not found: {resource_type} with ID '{id}'")]
    NotFound {
        /// Logical name of the resource type.
        resource_type: String,
        /// Identifier that was not found.
        id: String,
    },

    /// Request payload was rejected.
    #[error("Bad request: {message}")]
    BadRequest {
        /// Human-readable reason.
        message: String,
    },

    /// Authentication failed or was absent.
    #[error("Unauthorized access: {reason}")]
    Unauthorized {
        /// Human-readable reason.
        reason: String,
    },

    /// Authenticated principal lacks permission.
    #[error("Access forbidden: {resource} - {reason}")]
    Forbidden {
        /// Resource being accessed.
        resource: String,
        /// Human-readable reason.
        reason: String,
    },

    /// Field-level validation failed.
    #[error("Validation failed for field '{field}': {reason}")]
    ValidationError {
        /// Offending field path.
        field: String,
        /// Human-readable reason.
        reason: String,
    },

    /// Operation conflicted with existing state.
    #[error("Conflict: {resource} already exists")]
    ConflictError {
        /// Resource that already exists.
        resource: String,
    },

    /// Caller exceeded a rate limit.
    #[error("Rate limit exceeded for {resource}")]
    RateLimited {
        /// Resource being rate-limited.
        resource: String,
    },

    /// Downstream service was unavailable.
    #[error("Service temporarily unavailable: {service}")]
    ServiceUnavailable {
        /// Name of the unavailable service.
        service: String,
    },

    /// Database call failed.
    #[error("Database operation failed: {message}")]
    DatabaseError {
        /// Human-readable reason.
        message: String,
    },

    /// JSON serialization failed.
    #[error("JSON serialization failed")]
    JsonError(#[from] serde_json::Error),

    /// Authentication subsystem failed.
    #[error("Authentication token error: {message}")]
    AuthenticationError {
        /// Human-readable reason.
        message: String,
    },

    /// Catch-all internal failure.
    #[error("Internal server error: {message}")]
    InternalError {
        /// Human-readable reason.
        message: String,
    },
}

impl InternalApiError {
    /// Build a `NotFound` variant.
    pub fn not_found(resource_type: impl Into<String>, id: impl Into<String>) -> Self {
        Self::NotFound {
            resource_type: resource_type.into(),
            id: id.into(),
        }
    }

    /// Build a `BadRequest` variant.
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest {
            message: message.into(),
        }
    }

    /// Build an `Unauthorized` variant.
    pub fn unauthorized(reason: impl Into<String>) -> Self {
        Self::Unauthorized {
            reason: reason.into(),
        }
    }

    /// Build a `Forbidden` variant.
    pub fn forbidden(resource: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Forbidden {
            resource: resource.into(),
            reason: reason.into(),
        }
    }

    /// Build a `ValidationError` variant.
    pub fn validation_error(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::ValidationError {
            field: field.into(),
            reason: reason.into(),
        }
    }

    /// Build a `ConflictError` variant.
    pub fn conflict(resource: impl Into<String>) -> Self {
        Self::ConflictError {
            resource: resource.into(),
        }
    }

    /// Build a `RateLimited` variant.
    pub fn rate_limited(resource: impl Into<String>) -> Self {
        Self::RateLimited {
            resource: resource.into(),
        }
    }

    /// Build a `ServiceUnavailable` variant.
    pub fn service_unavailable(service: impl Into<String>) -> Self {
        Self::ServiceUnavailable {
            service: service.into(),
        }
    }

    /// Build an `InternalError` variant.
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::InternalError {
            message: message.into(),
        }
    }

    /// Build a `DatabaseError` variant.
    pub fn database_error(message: impl Into<String>) -> Self {
        Self::DatabaseError {
            message: message.into(),
        }
    }

    /// Build an `AuthenticationError` variant.
    pub fn authentication_error(message: impl Into<String>) -> Self {
        Self::AuthenticationError {
            message: message.into(),
        }
    }

    /// Map this variant to its [`ErrorCode`].
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
