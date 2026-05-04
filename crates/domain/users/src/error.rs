//! Typed error surface for the users crate.

use systemprompt_identifiers::UserId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UserError {
    #[error("user not found: {0}")]
    NotFound(UserId),

    #[error("user already exists with email: {0}")]
    EmailAlreadyExists(String),

    #[error("invalid status: {0}")]
    InvalidStatus(String),

    #[error("invalid role: {0}")]
    InvalidRole(String),

    #[error("invalid roles: {0:?}")]
    InvalidRoles(Vec<String>),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("repository error: {0}")]
    Repository(#[from] systemprompt_database::RepositoryError),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("pool error: {0}")]
    Pool(String),
}

pub type Result<T> = std::result::Result<T, UserError>;

pub type UserResult<T> = Result<T>;
