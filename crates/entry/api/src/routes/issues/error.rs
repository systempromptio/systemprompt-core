//! The one mapping of [`IssuesError`] onto the HTTP error envelope: issue
//! reports switched off or a report the caller cannot see is 404, an
//! invalid field 422, the hourly filing limit 429, a closed report, a full
//! attachment list or an upload not yet completed 409, each with its
//! `error_key`. Every other failure is the fixed 5xx body with its cause
//! logged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_files::issues::IssuesError;
use systemprompt_models::api::ApiError;

use crate::error::ApiHttpError;

impl From<IssuesError> for ApiHttpError {
    fn from(error: IssuesError) -> Self {
        let api = match error {
            IssuesError::Disabled => {
                ApiError::not_found(error.to_string()).with_error_key("issues_disabled")
            },
            IssuesError::Invalid { .. } => {
                ApiError::validation_error(error.to_string(), Vec::new())
                    .with_error_key("invalid_issue")
            },
            IssuesError::RateLimited { .. } => {
                ApiError::rate_limited(error.to_string()).with_error_key("issue_rate_limited")
            },
            IssuesError::NotFound(_) => {
                ApiError::not_found(error.to_string()).with_error_key("issue_not_found")
            },
            IssuesError::NotActive(_) => {
                ApiError::conflict(error.to_string()).with_error_key("issue_not_active")
            },
            IssuesError::AttachmentsFull { .. } => {
                ApiError::conflict(error.to_string()).with_error_key("issue_attachments_full")
            },
            IssuesError::UploadNotReady(_) => {
                ApiError::conflict(error.to_string()).with_error_key("upload_not_ready")
            },
            IssuesError::Repository(_)
            | IssuesError::Database(_)
            | IssuesError::Notifications(_)
            | IssuesError::NotificationKind(_) => {
                ApiError::internal("issue report operation failed", error)
            },
        };
        Self::from(api)
    }
}
