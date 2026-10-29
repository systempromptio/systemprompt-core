//! Issue reports: the bug and feedback reports a user or an agent files.
//!
//! A report folds repeats by fingerprint while active, carries
//! document-store upload blobs as attachments (which the blob collector keeps
//! while a report holds them), is announced on the `issues` notification
//! stream, and is purged `retentionDays` after it is resolved or closed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod error;
mod filing;
pub mod jobs;
pub mod repository;
pub mod service;

pub use error::{IssuesError, IssuesResult};
pub use jobs::IssueRetentionJob;
pub use repository::{IssueListQuery, IssueReportRepository};
pub use service::{
    FiledIssue, ISSUE_SUBJECT, ISSUES_STREAM, IssueFiling, IssueService, fingerprint,
};
