//! Typed error surface for the users crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_database::IdentifierError;
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

        #[error("purge {kind} {name} is not a safe SQL identifier")]
        PurgeIdentifier {
            kind: &'static str,
            name: String,
            #[source]
            source: IdentifierError,
        },

        #[error("user {0} is under legal hold and cannot be purged")]
        LegalHold(UserId),

        #[error("user {0} is not archived; archive before purging")]
        NotArchived(UserId),

        #[error("user {id} cannot be restored: not archived, or archived more than {window_days} days ago")]
        RestoreRefused { id: UserId, window_days: u32 },

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
