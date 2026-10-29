//! Issue report failures: what a reporter or triager is refused, and the
//! repository, notification and database errors underneath.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::NotificationSinkError;
use systemprompt_identifiers::{DocumentUploadId, IssueReportId};
use systemprompt_models::notifications::NotificationKindError;
use systemprompt_traits::RepositoryError;

#[derive(Debug, thiserror::Error)]
pub enum IssuesError {
    #[error("issue reports are not enabled (files.issues.enabled)")]
    Disabled,
    #[error("{field}: {reason}")]
    Invalid { field: &'static str, reason: String },
    #[error("at most {limit} new issue reports an hour; this reporter has reached it")]
    RateLimited { limit: u32 },
    #[error("issue report {0} not found")]
    NotFound(IssueReportId),
    #[error("issue report {0} is resolved or closed and takes no attachments")]
    NotActive(IssueReportId),
    #[error("issue report {report} already carries its {limit} attachments")]
    AttachmentsFull { report: IssueReportId, limit: u32 },
    #[error("upload {0} is not a completed upload of this reporter")]
    UploadNotReady(DocumentUploadId),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Notifications(#[from] NotificationSinkError),
    #[error(transparent)]
    NotificationKind(#[from] NotificationKindError),
}

pub type IssuesResult<T> = Result<T, IssuesError>;
