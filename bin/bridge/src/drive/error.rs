//! Failures of a Drive sync step.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use systemprompt_identifiers::{ContentHash, DocumentId};

#[derive(Debug, thiserror::Error)]
pub enum DriveError {
    #[error("{context} {}: {source}", path.display())]
    Io {
        context: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("drive state {} is corrupt and was moved to {}: {source}", path.display(), quarantined.display())]
    StateCorrupt {
        path: PathBuf,
        quarantined: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("drive payload could not be encoded: {0}")]
    Encode(#[source] serde_json::Error),
    #[error("gateway path {0:?} does not name a file inside the Drive folder")]
    UnsafePath(String),
    #[error("blob {expected} arrived with digest {actual}; it was not written")]
    HashMismatch {
        expected: ContentHash,
        actual: ContentHash,
    },
    #[error("change for document {document_id} at {path:?} carries no revision or digest")]
    IncompleteChange {
        document_id: DocumentId,
        path: String,
    },
    #[error("head of {path:?} is not in the gateway manifest")]
    HeadMissing { path: String },
    #[error("gateway has the document store disabled ({endpoint} answered 404)")]
    ServerDisabled { endpoint: &'static str },
    #[error("gateway rejected the credential at {endpoint}")]
    Unauthorized { endpoint: &'static str },
    #[error("this sign-in carries no device token for Drive sync; sign in again on this device")]
    DeviceTokenMissing,
    #[error("gateway answered {status} at {endpoint}: {rejection}")]
    Rejected {
        endpoint: &'static str,
        status: reqwest::StatusCode,
        rejection: Box<crate::gateway::GatewayRejection>,
    },
    #[error("network at {endpoint}: {source}")]
    Network {
        endpoint: &'static str,
        #[source]
        source: reqwest::Error,
    },
    #[error("gateway response at {endpoint} did not decode: {source}")]
    Decode {
        endpoint: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("drive device id: {0}")]
    DeviceId(#[source] systemprompt_identifiers::error::IdValidationError),
    #[error("gateway token: {0}")]
    Token(#[source] crate::proxy::forward::ForwardError),
    #[error("credential: {0}")]
    Credential(#[source] crate::auth::ChainError),
    #[error("config: {0}")]
    Config(#[source] crate::config::ConfigReadError),
}

impl DriveError {
    pub(crate) fn io(
        context: &'static str,
        path: impl Into<PathBuf>,
        source: std::io::Error,
    ) -> Self {
        Self::Io {
            context,
            path: path.into(),
            source,
        }
    }
}

pub type DriveResult<T> = Result<T, DriveError>;
