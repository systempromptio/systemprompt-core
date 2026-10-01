//! Typed error surface for the users crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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

        #[error("account merge is unavailable: no owner reassignments are configured")]
        MergeUnavailable,

        #[error("owner reassignment failed in the {domain} domain: {source}")]
        OwnerReassignment {
            domain: &'static str,
            #[source]
            source: systemprompt_traits::RepositoryError,
        },
    }
}

impl From<sqlx::Error> for UserError {
    fn from(err: sqlx::Error) -> Self {
        Self::Repository(systemprompt_traits::RepositoryError::from(err))
    }
}

pub type Result<T> = std::result::Result<T, UserError>;

pub type UserResult<T> = Result<T>;
