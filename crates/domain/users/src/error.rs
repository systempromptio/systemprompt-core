//! Typed error surface for the users crate.

use systemprompt_identifiers::UserId;
use thiserror::Error;

/// Errors produced by user, API-key, device-cert, and IP-ban code paths.
#[derive(Debug, Error)]
pub enum UserError {
    /// User row does not exist for the given typed `UserId`.
    #[error("user not found: {0}")]
    NotFound(UserId),

    /// Attempted to create a user whose email is already taken.
    #[error("user already exists with email: {0}")]
    EmailAlreadyExists(String),

    /// Status string in the database row could not be parsed into a typed
    /// status.
    #[error("invalid status: {0}")]
    InvalidStatus(String),

    /// Role string could not be parsed into a typed role.
    #[error("invalid role: {0}")]
    InvalidRole(String),

    /// One or more role strings could not be parsed.
    #[error("invalid roles: {0:?}")]
    InvalidRoles(Vec<String>),

    /// `SQLx` error bubbled up from a repository call.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Validation failure — e.g. malformed email or password policy violation.
    #[error("validation error: {0}")]
    Validation(String),

    /// Failure resolving a database pool from the runtime
    /// [`systemprompt_database::DbPool`].
    #[error("pool error: {0}")]
    Pool(String),
}

impl From<anyhow::Error> for UserError {
    fn from(err: anyhow::Error) -> Self {
        Self::Pool(err.to_string())
    }
}

/// Convenience alias for `Result<T, UserError>`.
pub type Result<T> = std::result::Result<T, UserError>;

/// Public name of the result alias for downstream callers that prefer an
/// explicit, non-ambiguous identifier.
pub type UserResult<T> = Result<T>;
