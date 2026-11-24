//! Typed error surface for the users crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_database::IdentifierError;
use systemprompt_identifiers::{ExtensionId, UserId};
use systemprompt_models::domain_error;

domain_error! {
    pub enum UserError {
        common: [repository, validation],

        #[error("user not found: {0}")]
        NotFound(UserId),

        #[error("user already exists with email: {email}")]
        EmailAlreadyExists { email: String },

        #[error("user {email} exists with a different account kind; the declaration cannot convert it")]
        AccountKindMismatch { email: String },

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

        #[error("api key expiry {requested} exceeds the {max_days}-day maximum lifetime")]
        ApiKeyLifetimeExceeded {
            requested: chrono::DateTime<chrono::Utc>,
            max_days: u32,
        },

        #[error("federated sign-in refused: {0}")]
        FederatedRefused(systemprompt_traits::FederatedRefusal),

        #[error("account merge is unavailable: no owner reassignments are configured")]
        MergeUnavailable,

        #[error(
            "owner reassignment over security table {table} declared by '{owner}' is refused; its \
             rows die with the source user"
        )]
        SecurityTableReassignment {
            owner: ExtensionId,
            table: &'static str,
        },

        #[error("owner reassignment of {table}.{column} declared by '{owner}' failed")]
        OwnerReassignment {
            owner: ExtensionId,
            table: &'static str,
            column: &'static str,
            #[source]
            source: Box<sqlx::Error>,
        },
    }
}

impl From<sqlx::Error> for UserError {
    fn from(err: sqlx::Error) -> Self {
        Self::Repository(systemprompt_traits::RepositoryError::from(err))
    }
}

impl From<UserError> for systemprompt_models::api::ApiError {
    fn from(err: UserError) -> Self {
        match err {
            UserError::Repository(inner) => Self::from(inner),
            e @ UserError::NotFound(_) => Self::not_found(e.to_string()),
            e @ UserError::FederatedRefused(_) => Self::forbidden(e.to_string()),
            e @ (UserError::EmailAlreadyExists { .. }
            | UserError::AccountKindMismatch { .. }
            | UserError::LegalHold(_)
            | UserError::NotArchived(_)
            | UserError::RestoreRefused { .. }) => Self::conflict(e.to_string()),
            e @ (UserError::Validation(_)
            | UserError::InvalidRoles(_)
            | UserError::ApiKeyLifetimeExceeded { .. }) => Self::bad_request(e.to_string()),
            e @ (UserError::MergeUnavailable
            | UserError::SecurityTableReassignment { .. }
            | UserError::PurgeIdentifier { .. }
            | UserError::OwnerReassignment { .. }) => Self::internal("User operation failed", e),
        }
    }
}

pub type Result<T> = std::result::Result<T, UserError>;

pub type UserResult<T> = Result<T>;
