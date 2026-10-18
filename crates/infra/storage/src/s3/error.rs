//! Errors raised by the S3-compatible backend.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_traits::{FileStorageError, StoredFileId};

/// A failed S3 call.
#[derive(Debug, thiserror::Error)]
pub enum S3Error {
    #[error("S3 request: {0}")]
    Http(#[from] reqwest::Error),

    #[error("S3 answered {status}: {body}")]
    Status { status: u16, body: String },

    #[error("S3 URL: {0}")]
    Url(#[from] url::ParseError),

    #[error("S3 endpoint {0} has no host")]
    EndpointHost(String),

    #[error("S3 signing key: {0}")]
    SigningKey(#[source] hmac::digest::InvalidLength),

    #[error("S3 object {name} has an unreadable content-length: {source}")]
    ObjectSize {
        name: String,
        #[source]
        source: std::num::ParseIntError,
    },

    #[error("S3 object {name} has an unreadable {header} header")]
    ObjectHeader { name: String, header: &'static str },
}

impl S3Error {
    pub(crate) fn into_storage(self, id: &StoredFileId) -> FileStorageError {
        match self {
            Self::Status { status: 404, .. } => FileStorageError::NotFound(id.as_str().to_owned()),
            other => FileStorageError::Backend(Box::new(other)),
        }
    }
}
