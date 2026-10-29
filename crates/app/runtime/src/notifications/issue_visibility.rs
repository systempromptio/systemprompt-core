//! Per-reader visibility of issue reports for notifications.
//!
//! A report is visible, with its excerpt, to its reporter and to an active
//! user holding one of `files.issues.triageRoles`; it is hidden from
//! everyone else, and a report that no longer exists is hidden.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::{ResourceVisibility, Visibility, VisibilityError};
use systemprompt_files::issues::{ISSUE_SUBJECT, IssueService, IssuesError};
use systemprompt_identifiers::{IssueReportId, UserId};
use systemprompt_models::notifications::SubjectRef;
use systemprompt_users::{UserError, UserRepository};

const VISIBLE: Visibility = Visibility::Visible {
    excerpt_allowed: true,
};

/// The [`ResourceVisibility`] for `issue_report` subjects.
#[derive(Debug, Clone)]
pub struct IssueVisibility {
    issues: Arc<IssueService>,
    users: Arc<UserRepository>,
}

impl IssueVisibility {
    #[must_use]
    pub const fn new(issues: Arc<IssueService>, users: Arc<UserRepository>) -> Self {
        Self { issues, users }
    }

    async fn is_triager(&self, reader: &UserId) -> Result<bool, VisibilityError> {
        for role in &self.issues.config().triage_roles {
            if self
                .users
                .is_active_with_role(reader, role)
                .await
                .map_err(user_failed)?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

fn issue_failed(error: IssuesError) -> VisibilityError {
    VisibilityError::Other {
        kind: ISSUE_SUBJECT.to_owned(),
        source: Box::new(error),
    }
}

fn user_failed(error: UserError) -> VisibilityError {
    VisibilityError::Other {
        kind: ISSUE_SUBJECT.to_owned(),
        source: Box::new(error),
    }
}

#[async_trait]
impl ResourceVisibility for IssueVisibility {
    fn kinds(&self) -> &'static [&'static str] {
        &[ISSUE_SUBJECT]
    }

    async fn visible(
        &self,
        subject: &SubjectRef,
        reader: &UserId,
    ) -> Result<Visibility, VisibilityError> {
        if subject.kind.as_str() != ISSUE_SUBJECT {
            return Ok(Visibility::Hidden);
        }
        let report = self
            .issues
            .find(&IssueReportId::new(subject.id.as_str()))
            .await
            .map_err(issue_failed)?;
        let Some(report) = report else {
            return Ok(Visibility::Hidden);
        };
        if &report.reporter_user_id == reader || self.is_triager(reader).await? {
            Ok(VISIBLE)
        } else {
            Ok(Visibility::Hidden)
        }
    }
}
