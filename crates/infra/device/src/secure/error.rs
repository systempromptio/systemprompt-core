//! The crate's error taxonomy. No variant carries plaintext or ciphertext
//! bytes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::path::PathBuf;

/// Why a sealing, keystore or file operation failed.
///
/// [`SecureError::Unavailable`] and [`SecureError::Unsupported`] mean the
/// ciphertext may still be good and must be kept; [`SecureError::Rejected`]
/// means it will never open here.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SecureError {
    #[error("secure storage is unavailable right now: {source}")]
    Unavailable {
        #[source]
        source: io::Error,
    },
    #[error("secure storage is not available in this build or on this platform")]
    Unsupported,
    #[error("sealed data was not produced for this user and app, or is corrupt: {source}")]
    Rejected {
        #[source]
        source: io::Error,
    },
    #[error("sealed value is not valid base64")]
    Encoding {
        #[source]
        source: base64::DecodeError,
    },
    #[error("sealed value is empty")]
    Empty,
    #[error("opened secret is not valid UTF-8")]
    NotUtf8,
    #[error("credential store: {source}")]
    Keystore {
        #[source]
        source: io::Error,
    },
    #[error("{op} {}: {source}", path.display())]
    Io {
        op: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("access control of {}: {source}", path.display())]
    Acl {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("refusing to go through a link or reparse point at {}", path.display())]
    Link { path: PathBuf },
    #[error("{relative:?} is not a safe path inside {}: {reason}", root.display())]
    Escapes {
        root: PathBuf,
        relative: String,
        reason: &'static str,
    },
    #[error("{} did not read back as written", path.display())]
    ReadbackMismatch { path: PathBuf },
    #[error("{} still exists after removal", path.display())]
    StillPresent { path: PathBuf },
}

impl SecureError {
    #[must_use]
    pub const fn keeps_ciphertext(&self) -> bool {
        matches!(self, Self::Unavailable { .. } | Self::Unsupported)
    }

    pub(crate) fn io(op: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            op,
            path: path.into(),
            source,
        }
    }

    pub(crate) fn rejected(reason: &'static str) -> Self {
        Self::Rejected {
            source: io::Error::new(io::ErrorKind::InvalidData, reason),
        }
    }
}
