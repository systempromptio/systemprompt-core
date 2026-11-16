//! Per-reader visibility of issue reports for notifications.
//!
//! A report is visible, with its excerpt, to its reporter and to an active
//! user holding one of `files.issues.triageRoles`; it is hidden from
//! everyone else, and a report that no longer exists is hidden. Role
//! membership is asked of the user-role audience resolver the `users`
//! member provides, so files names no users type. The files member provides
//! it as the `CoreSeam<dyn ResourceVisibility, IssueVisibility>` the
//! notifications slot appends.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::{
    Membership, NotificationAudienceError, NotificationAudienceResolver, ResourceVisibility,
    Visibility, VisibilityError,
};
use systemprompt_identifiers::{IssueReportId, UserId};
use systemprompt_models::notifications::{AudienceSubject, SubjectRef};

use super::{ISSUE_SUBJECT, IssueService, IssuesError};

const VISIBLE: Visibility = Visibility::Visible {
    excerpt_allowed: true,
};

/// The [`ResourceVisibility`] for `issue_report` subjects.
#[derive(Clone)]
pub struct IssueVisibility {
    issues: Arc<IssueService>,
    roles: Arc<dyn NotificationAudienceResolver>,
}

impl std::fmt::Debug for IssueVisibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IssueVisibility")
            .field("roles", &self.roles.name())
            .finish_non_exhaustive()
    }
}

impl IssueVisibility {
    #[must_use]
    pub fn new(issues: Arc<IssueService>, roles: Arc<dyn NotificationAudienceResolver>) -> Self {
        Self { issues, roles }
    }

    async fn is_triager(&self, reader: &UserId) -> Result<bool, VisibilityError> {
        for role in &self.issues.config().triage_roles {
            let held = self
                .roles
                .is_member(&AudienceSubject::Role(role.clone()), reader)
                .await
                .map_err(role_failed)?;
            if held == Membership::Member {
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

fn role_failed(error: NotificationAudienceError) -> VisibilityError {
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
