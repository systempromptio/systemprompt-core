use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error("Constraint violation: {0}")]
    Constraint(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl RepositoryError {
    pub fn not_found<T: std::fmt::Display>(id: T) -> Self {
        Self::NotFound(id.to_string())
    }

    pub fn constraint<T: Into<String>>(message: T) -> Self {
        Self::Constraint(message.into())
    }

    pub fn invalid_argument<T: Into<String>>(message: T) -> Self {
        Self::InvalidArgument(message.into())
    }

    pub fn internal<T: Into<String>>(message: T) -> Self {
        Self::Internal(message.into())
    }

    #[must_use]
    pub const fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound(_))
    }

    #[must_use]
    pub const fn is_constraint(&self) -> bool {
        matches!(self, Self::Constraint(_))
    }
}

impl From<anyhow::Error> for RepositoryError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}
