//! The one mapping of [`UploadError`] onto the HTTP error envelope: the
//! error's stable code is the `error_key`, and the status follows the
//! failure — 400 for a malformed call, 404 for an upload the caller cannot
//! see, 409 for a closed upload or missing parts, 413 past a size limit,
//! 422 for refused content, 429 past the quota.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_files::uploads::UploadError;
use systemprompt_models::api::{ApiError, ErrorCode};

use crate::error::ApiHttpError;

impl From<UploadError> for ApiHttpError {
    fn from(error: UploadError) -> Self {
        let code = match &error {
            UploadError::Invalid(_)
            | UploadError::PartOutOfRange { .. }
            | UploadError::PartSize { .. } => ErrorCode::BadRequest,
            UploadError::NotFound | UploadError::BlobNotOwned(_) => ErrorCode::NotFound,
            UploadError::Closed { .. } | UploadError::MissingParts { .. } => {
                ErrorCode::ConflictError
            },
            UploadError::TooLarge { .. } => ErrorCode::PayloadTooLarge,
            UploadError::QuotaExceeded { .. }
            | UploadError::TooManyOpen { .. }
            | UploadError::PartBudgetExhausted { .. } => ErrorCode::RateLimited,
            UploadError::Refused { .. } => ErrorCode::ValidationError,
            UploadError::Storage(_)
            | UploadError::Repository(_)
            | UploadError::Blob(_)
            | UploadError::Interrupted(_)
            | UploadError::Unanswered(_) => {
                return Self::from(ApiError::internal("upload storage failed", error));
            },
        };
        let key = error.code();
        Self::from(ApiError::new(code, error.to_string()).with_error_key(key))
    }
}
