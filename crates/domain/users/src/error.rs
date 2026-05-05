//! Typed error surface for the users crate.

use systemprompt_identifiers::UserId;
use systemprompt_models::domain_error;

domain_error! {
    pub enum UserError {
        common: [repository, validation],

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

        #[error("pool error: {0}")]
        Pool(String),
    }
}

impl From<sqlx::Error> for UserError {
    fn from(err: sqlx::Error) -> Self {
        Self::Repository(systemprompt_database::RepositoryError::from(err))
    }
}

pub type Result<T> = std::result::Result<T, UserError>;

pub type UserResult<T> = Result<T>;
