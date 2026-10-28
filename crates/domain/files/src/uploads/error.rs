//! [`UploadError`]: why an upload call failed, and [`RefusalReason`], why
//! its content was refused.
//!
//! A refusal is final: the session is closed as `refused`, its stored
//! bytes are deleted and the reason is kept on the row. Every other client
//! error leaves the session open so the caller can correct and retry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ContentHash;
use systemprompt_traits::{FileStorageError, RepositoryError};

use super::models::UploadStatus;
use crate::documents::blob_store::BlobStoreError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalReason {
    ChecksumMismatch,
    MimeMismatch,
    UnsupportedType,
    ArchiveLimits,
    ImageLimits,
    UnsafeSvg,
    UncheckedFormat,
}

impl RefusalReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ChecksumMismatch => "checksum_mismatch",
            Self::MimeMismatch => "mime_mismatch",
            Self::UnsupportedType => "unsupported_type",
            Self::ArchiveLimits => "archive_limits",
            Self::ImageLimits => "image_limits",
            Self::UnsafeSvg => "unsafe_svg",
            Self::UncheckedFormat => "unchecked_format",
        }
    }
}

impl std::fmt::Display for RefusalReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UploadError {
    #[error("{0}")]
    Invalid(String),
    #[error("upload not found")]
    NotFound,
    #[error("upload is {status}, not open")]
    Closed { status: UploadStatus },
    #[error("upload of {size} bytes exceeds the {limit}-byte limit")]
    TooLarge { size: u64, limit: u64 },
    #[error(
        "{used} bytes uploaded in the last day; {requested} more exceeds the {limit}-byte quota"
    )]
    QuotaExceeded {
        used: u64,
        requested: u64,
        limit: u64,
    },
    #[error("this upload has received its {limit}-byte part budget; complete it or start over")]
    PartBudgetExhausted { limit: u64 },
    #[error("{open} uploads are already open; the limit is {limit}")]
    TooManyOpen { open: u64, limit: u64 },
    #[error("part {part} is outside 1..={parts}")]
    PartOutOfRange { part: u32, parts: u32 },
    #[error("part {part} must be {expected} bytes, got {actual}")]
    PartSize {
        part: u32,
        expected: u64,
        actual: u64,
    },
    #[error("parts {missing:?} have not been uploaded")]
    MissingParts { missing: Vec<u32> },
    #[error("upload refused ({reason}): {detail}")]
    Refused {
        reason: RefusalReason,
        detail: String,
    },
    #[error("blob {0} was not uploaded by the caller")]
    BlobNotOwned(ContentHash),
    #[error("upload storage failed")]
    Storage(#[from] FileStorageError),
    #[error("the upload completion stopped")]
    Interrupted(#[from] tokio::task::JoinError),
    #[error("upload repository failed")]
    Repository(#[from] RepositoryError),
    #[error("upload blob store failed")]
    Blob(#[from] BlobStoreError),
}

impl UploadError {
    pub(crate) fn refused(reason: RefusalReason, detail: impl Into<String>) -> Self {
        Self::Refused {
            reason,
            detail: detail.into(),
        }
    }

    /// The stable code a transport answers this error with.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Invalid(_) | Self::PartOutOfRange { .. } | Self::PartSize { .. } => {
                "invalid_request"
            },
            Self::NotFound | Self::BlobNotOwned(_) => "not_found",
            Self::Closed { .. } => "upload_closed",
            Self::TooLarge { .. } => "too_large",
            Self::QuotaExceeded { .. } => "quota_exceeded",
            Self::TooManyOpen { .. } => "too_many_open_uploads",
            Self::PartBudgetExhausted { .. } => "part_budget_exhausted",
            Self::MissingParts { .. } => "missing_parts",
            Self::Refused { reason, .. } => reason.as_str(),
            Self::Storage(_) | Self::Repository(_) | Self::Blob(_) | Self::Interrupted(_) => {
                "internal"
            },
        }
    }
}

pub type UploadResult<T> = Result<T, UploadError>;
