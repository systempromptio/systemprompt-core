use systemprompt_identifiers::UserId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UserError {
    #[error("User not found: {0}")]
    NotFound(UserId),

    #[error("User already exists with email: {0}")]
    EmailAlreadyExists(String),

    #[error("Invalid status: {0}")]
    InvalidStatus(String),

    #[error("Invalid role: {0}")]
    InvalidRole(String),

    #[error("Invalid roles: {0:?}")]
    InvalidRoles(Vec<String>),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Validation error: {0}")]
    Validation(String),
}

pub type Result<T> = std::result::Result<T, UserError>;
