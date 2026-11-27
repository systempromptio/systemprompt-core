//! Failures inside the sync engine. Only `SyncError` leaves the crate; the
//! others become a sync status or a log line. None of them carries a
//! credential.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use sp_accounts::AccountsError;
use sp_client::ClientError;
use sp_extension::ExtensionError;
use sp_projection::{MaterializeError, ProjectionError};
use sp_protocol::view::SyncErrorKind;

use crate::acceptance::Refusal;

/// Why the sync engine could not start.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("the sync engine must be started inside a tokio runtime")]
    NoRuntime(#[source] tokio::runtime::TryCurrentError),
}

/// Why accepted content could not reach the applier or the extensions.
#[derive(Debug, thiserror::Error)]
pub enum TargetError {
    #[error("the projection could not be applied")]
    Projection(#[source] ProjectionError),
    #[error("the runtime files could not be written")]
    Materialize(#[source] MaterializeError),
    #[error("an extension refused the applied instance")]
    Extension(#[source] ExtensionError),
    #[error("the account could not be read")]
    Accounts(#[source] AccountsError),
    #[error("the blocking filesystem job did not finish")]
    Blocking(#[source] tokio::task::JoinError),
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum PollError {
    #[error("the instance call failed")]
    Client(#[source] ClientError),
    #[error("the manifest is refused")]
    Refused(#[source] Refusal),
    #[error("the instance serves a manifest key the user has not trusted")]
    KeyChanged,
    #[error("the instance answered 304 before any manifest was accepted")]
    NoManifest,
    #[error("the account could not be read or written")]
    Accounts(#[source] AccountsError),
    #[error("the accepted content could not be applied")]
    Apply(#[source] TargetError),
}

impl PollError {
    pub(crate) fn kind(&self) -> SyncErrorKind {
        match self {
            Self::Client(error) => error.sync_error_kind(),
            Self::Refused(_) | Self::NoManifest => SyncErrorKind::Invalid,
            Self::KeyChanged => SyncErrorKind::KeyChanged,
            Self::Accounts(error) => accounts_kind(error),
            Self::Apply(_) => SyncErrorKind::Service,
        }
    }

    pub(crate) const fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Client(error) => error.retry_after(),
            _ => None,
        }
    }
}

impl From<AccountsError> for PollError {
    fn from(error: AccountsError) -> Self {
        Self::Accounts(error)
    }
}

fn accounts_kind(error: &AccountsError) -> SyncErrorKind {
    match error {
        AccountsError::UnknownAccount { .. }
        | AccountsError::NotSignedIn { .. }
        | AccountsError::Credential(_) => SyncErrorKind::Refused,
        AccountsError::SecureStorageUnavailable(_) => SyncErrorKind::Insecure,
        AccountsError::Client(inner) => inner.sync_error_kind(),
        _ => SyncErrorKind::Service,
    }
}
