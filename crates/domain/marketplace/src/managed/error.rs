//! Typed failures preserve validation, ownership and conflict distinctions.

#[derive(Debug, thiserror::Error)]
pub enum ManagedError {
    #[error("Invalid managed resource: {0}")]
    Invalid(String),
    #[error("Managed resource is unavailable in this scope")]
    Unavailable,
    #[error("Managed resource conflict: {0}")]
    Conflict(String),
    #[error("Managed resource integrity check failed")]
    Integrity,
    #[error("Managed resource storage failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Managed resource serialization failed: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, ManagedError>;

pub(super) fn invalid(message: &str) -> ManagedError {
    ManagedError::Invalid(message.to_owned())
}
