//! Typed failures of the gateway audit trail and its settlement journal.
//!
//! [`GatewayAuditError`] covers admission, completion and settlement of a
//! gateway request's audit row and the encrypted receipt that backs it. An
//! invariant the journal refuses to break is an
//! [`GatewayAuditError::Invariant`] carrying the fixed message; every other
//! variant keeps its cause as the error source.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_config::{SecretsBootstrapError, StateDirError};
use systemprompt_loader::ConfigLoadError;
use systemprompt_traits::{ContextProviderError, RepositoryError};

use crate::pricing::MissingPricing;

pub type GatewayAuditResult<T> = Result<T, GatewayAuditError>;

/// A failure admitting, completing or settling a gateway audit record.
#[derive(Debug, thiserror::Error)]
pub enum GatewayAuditError {
    #[error("{0}")]
    Invariant(&'static str),

    #[error(
        "Gateway accounting journal requires the `encryption_master_key` secret (32 bytes as 64 \
         hex characters); with `secrets.source: env` it must also be listed in \
         SYSTEMPROMPT_CUSTOM_SECRETS"
    )]
    MissingJournalKey,

    #[error(transparent)]
    JournalKey(#[from] SecretsBootstrapError),

    #[error("Cannot create gateway journal at {}", .0.dir.display())]
    JournalDir(#[source] StateDirError),

    #[error("Journal encryption failed")]
    Encrypt(#[source] chacha20poly1305::aead::Error),

    #[error("Journal authentication failed")]
    Authenticate(#[source] chacha20poly1305::aead::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    TryLock(#[from] std::fs::TryLockError),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Repository(#[from] RepositoryError),

    #[error(transparent)]
    Context(#[from] ContextProviderError),

    #[error(transparent)]
    Services(#[from] ConfigLoadError),

    #[error(transparent)]
    Pricing(#[from] MissingPricing),

    #[error("Gateway journal task failed")]
    Join(#[from] tokio::task::JoinError),
}

pub(crate) const fn ensure(condition: bool, message: &'static str) -> GatewayAuditResult<()> {
    if condition {
        Ok(())
    } else {
        Err(GatewayAuditError::Invariant(message))
    }
}
