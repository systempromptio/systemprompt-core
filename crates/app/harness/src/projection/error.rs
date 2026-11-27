//! Failures of the applier and of the runtime-file writer. None of them carries
//! file contents or a credential.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::path::PathBuf;

use systemprompt_device::secure::SecureError;

/// Why an apply or a clear stopped. Files written before the failure stay as
/// written; the next apply converges.
#[derive(Debug, thiserror::Error)]
pub enum ProjectionError {
    #[error("{} is not a valid ledger; refusing to treat a corrupt ledger as empty", .path.display())]
    LedgerJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("{} lists {entry:?}, which the projection never writes; refusing to act on it", .path.display())]
    LedgerEntry { path: PathBuf, entry: String },
    #[error("{path:?} is not a path the projection may write")]
    Unowned { path: String },
    #[error("{} is not UTF-8; refusing to rewrite it", .path.display())]
    NotUtf8 {
        path: PathBuf,
        #[source]
        source: std::str::Utf8Error,
    },
    #[error("cannot encode {what}")]
    Encode {
        what: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("cannot {op} {}", .path.display())]
    Io {
        op: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot {op}")]
    Secure {
        op: &'static str,
        #[source]
        source: SecureError,
    },
    #[error("cannot update the global settings file")]
    Settings(#[source] systemprompt_engine::config::ConfigError),
}

impl ProjectionError {
    pub(crate) fn secure(op: &'static str) -> impl FnOnce(SecureError) -> Self {
        move |source| Self::Secure { op, source }
    }
}

/// Why a skill's runtime files were not written or read back.
#[derive(Debug, thiserror::Error)]
pub enum MaterializeError {
    #[error("skill directory {dir:?} is not one the applier wrote for the applied instance")]
    NotManaged { dir: String },
    #[error("the installation plan is refused: {reason}")]
    InvalidPlan { reason: &'static str },
    #[error("{} is not a runtime ledger the writer could have written: {reason}", .path.display())]
    Sidecar { path: PathBuf, reason: &'static str },
    #[error("{} is not a JSON list of paths", .path.display())]
    SidecarJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error(transparent)]
    Ledger(#[from] ProjectionError),
    #[error("cannot {op}")]
    Secure {
        op: &'static str,
        #[source]
        source: SecureError,
    },
    #[error("cannot {op} {}", .path.display())]
    Io {
        op: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot encode the runtime ledger")]
    Encode(#[source] serde_json::Error),
    #[error("the digest read back for {path} is not a SHA-256")]
    Digest { path: String },
}

impl MaterializeError {
    pub(crate) fn secure(op: &'static str) -> impl FnOnce(SecureError) -> Self {
        move |source| Self::Secure { op, source }
    }
}
