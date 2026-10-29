//! Scheduled maintenance for issue reports: [`IssueRetentionJob`] deletes
//! reports resolved or closed past the retention period.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod retention;

pub use retention::IssueRetentionJob;
