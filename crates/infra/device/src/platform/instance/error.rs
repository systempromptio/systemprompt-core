//! Why the single-instance lock or a hand-off failed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::path::{Path, PathBuf};

use crate::secure::SecureError;

/// Why one hand-off exchange was refused. The primary logs it and keeps
/// listening; the launch that sent it sees it as [`InstanceError::Rejected`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FrameError {
    #[error("a hand-off of {length} bytes is over the limit of {limit}")]
    TooLarge { length: u64, limit: u32 },
    #[error("the hand-off is not the expected document: {source}")]
    Malformed {
        #[source]
        source: serde_json::Error,
    },
    #[error("the hand-off was cut short: {source}")]
    Transport {
        #[source]
        source: io::Error,
    },
    #[error("the other side did not finish the hand-off in time")]
    TimedOut,
    #[error("the running instance did not acknowledge the hand-off")]
    NotAcknowledged,
}

/// Why the data root could not be claimed or the running instance reached.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum InstanceError {
    #[error("the single-instance lock needs a Tokio runtime: {source}")]
    NoRuntime {
        #[source]
        source: tokio::runtime::TryCurrentError,
    },
    #[error("data root: {0}")]
    DataRoot(#[from] SecureError),
    #[error("{op} {}: {source}", path.display())]
    Io {
        op: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{} is too long for a socket address ({limit} bytes)", path.display())]
    EndpointTooLong { path: PathBuf, limit: usize },
    #[error("the running instance does not answer at {}: {source}", endpoint.display())]
    Unreachable {
        endpoint: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("hand-off rejected: {source}")]
    Rejected {
        #[source]
        source: FrameError,
    },
}

impl InstanceError {
    pub(super) fn io(op: &'static str, path: &Path, source: io::Error) -> Self {
        Self::Io {
            op,
            path: path.to_path_buf(),
            source,
        }
    }
}
