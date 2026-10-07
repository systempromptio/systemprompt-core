//! Errors raised by the Cloud Storage backend and its token sources.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_traits::{BoxedSource, FileStorageError, StoredFileId};

/// A failed Cloud Storage call or token mint.
#[derive(Debug, thiserror::Error)]
pub enum GcsError {
    #[error("obtain a Cloud Storage access token: {0}")]
    Token(#[source] BoxedSource),

    #[error("Cloud Storage request: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Cloud Storage answered {status}: {body}")]
    Status { status: u16, body: String },

    #[error("Cloud Storage response body: {0}")]
    Decode(#[from] serde_json::Error),

    #[error("Cloud Storage object {name} has an unreadable size: {source}")]
    ObjectSize {
        name: String,
        #[source]
        source: std::num::ParseIntError,
    },

    #[error("Cloud Storage object {name} has no {field}")]
    ObjectField { name: String, field: &'static str },

    #[error("Cloud Storage URL: {0}")]
    Url(#[from] url::ParseError),
}

impl GcsError {
    pub(crate) fn into_storage(self, id: &StoredFileId) -> FileStorageError {
        match self {
            Self::Status { status: 404, .. } => FileStorageError::NotFound(id.as_str().to_owned()),
            other => FileStorageError::Backend(Box::new(other)),
        }
    }
}
