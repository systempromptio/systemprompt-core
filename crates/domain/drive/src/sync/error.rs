//! Failures of the local folder sync.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use systemprompt_device::client::{ClientError, CredentialError};
use systemprompt_device::secure::SecureError;

use super::names::PathRefusal;

/// Why a sync step, or starting the sync, failed. None carries a credential or
/// file contents.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("{op} {}", path.display())]
    Io {
        op: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("a protected file operation failed")]
    Secure(#[source] SecureError),
    #[error("the Drive index {} did not decode and was moved to {}", path.display(), quarantined.display())]
    IndexCorrupt {
        path: PathBuf,
        quarantined: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("the Drive index could not be encoded")]
    Encode(#[source] serde_json::Error),
    #[error("{path:?} does not name a file inside the Drive folder")]
    UnsafePath { path: String },
    #[error("{path:?} cannot be placed in the Drive folder: {refusal}")]
    Unplaceable { path: String, refusal: PathRefusal },
    #[error("content {expected} arrived with digest {actual}; it was not written")]
    HashMismatch { expected: String, actual: String },
    #[error("a change for {path:?} carries no revision or digest")]
    IncompleteChange { path: String },
    #[error("the head of {path:?} is not in the instance's manifest")]
    HeadMissing { path: String },
    #[error("the instance has no document store ({endpoint} answered 404)")]
    Disabled { endpoint: &'static str },
    #[error("this sign-in carries no device token for the Drive sync; sign in again")]
    DeviceTokenMissing,
    #[error("no device token is available")]
    Credential(#[source] CredentialError),
    #[error("{endpoint} answered {status}")]
    Rejected {
        endpoint: &'static str,
        status: u16,
        error_key: Option<String>,
    },
    #[error(transparent)]
    Client(#[from] ClientError),
    #[error("the folder {} already mirrors another instance", root.display())]
    RootTaken { root: PathBuf },
    #[error("the Drive folder must be an absolute path to a folder, not a link")]
    InvalidRoot { root: PathBuf },
}

impl SyncError {
    pub(crate) fn io(op: &'static str, path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            op,
            path: path.into(),
            source,
        }
    }

    pub(crate) const fn status(&self) -> Option<u16> {
        match self {
            Self::Rejected { status, .. } => Some(*status),
            Self::Client(error) => error.status(),
            _ => None,
        }
    }

    pub(crate) fn is_gone(&self) -> bool {
        self.status() == Some(404)
    }

    pub(crate) fn is_refusal(&self) -> bool {
        matches!(self, Self::Rejected { status, .. } if (400..500).contains(status) && *status != 408 && *status != 429)
    }

    pub(crate) const fn refuses_token(&self) -> bool {
        matches!(self, Self::Client(error) if error.refuses_token())
    }
}

impl From<SecureError> for SyncError {
    fn from(error: SecureError) -> Self {
        Self::Secure(error)
    }
}
