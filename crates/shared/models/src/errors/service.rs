//! Application-layer service umbrella error and its conversions into
//! the public [`crate::api::ApiError`] HTTP shape.

use systemprompt_traits::RepositoryError;

use crate::api::ApiError;

/// Application-layer umbrella error returned by the service tier when
/// orchestrating repositories and external systems.
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    /// An underlying repository call failed.
    #[error("repository error: {0}")]
    Repository(#[from] RepositoryError),

    /// Input failed business validation.
    #[error("validation error: {0}")]
    Validation(String),

    /// A higher-level invariant of the domain was violated.
    #[error("business logic error: {0}")]
    BusinessLogic(String),

    /// An external dependency failed.
    #[error("external service error: {0}")]
    External(String),

    /// A requested entity was not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// The operation conflicted with existing state.
    #[error("conflict: {0}")]
    Conflict(String),

    /// The principal is unauthenticated.
    #[error("unauthorized: {0}")]
    Unauthorized(String),

    /// The principal lacks permission.
    #[error("forbidden: {0}")]
    Forbidden(String),
}

impl From<ServiceError> for ApiError {
    fn from(err: ServiceError) -> Self {
        match err {
            ServiceError::Repository(e) => e.into(),
            ServiceError::Validation(msg) | ServiceError::BusinessLogic(msg) => {
                Self::bad_request(msg)
            },
            ServiceError::NotFound(msg) => Self::not_found(msg),
            ServiceError::External(msg) => {
                Self::internal_error(format!("External service error: {msg}"))
            },
            ServiceError::Conflict(msg) => Self::conflict(msg),
            ServiceError::Unauthorized(msg) => Self::unauthorized(msg),
            ServiceError::Forbidden(msg) => Self::forbidden(msg),
        }
    }
}

impl From<RepositoryError> for ApiError {
    fn from(err: RepositoryError) -> Self {
        match err {
            RepositoryError::NotFound(msg) => Self::not_found(msg),
            RepositoryError::InvalidData(msg) | RepositoryError::ConstraintViolation(msg) => {
                Self::bad_request(msg)
            },
            RepositoryError::Database(e) => Self::internal_error(format!("Database error: {e}")),
            RepositoryError::Serialization(e) => {
                Self::internal_error(format!("Serialization error: {e}"))
            },
            RepositoryError::Other(e) => Self::internal_error(format!("Error: {e}")),
            _ => Self::internal_error(format!("Repository error: {err}")),
        }
    }
}
