//! [`IssueService`]: filing, attaching, listing, triage and retention of
//! issue reports.
//!
//! A new report and its `issue.reported` fact commit together on the
//! `issues` stream, whose audience the install declares; a repeat of an
//! active report folds into it and announces nothing. A triage transition
//! and its `issue.status_changed` fact to the reporter commit together. A
//! report is visible to its reporter and to the triage roles only.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use systemprompt_extension::DynNotificationSink;
use systemprompt_identifiers::{DocumentUploadId, IssueReportId, NotificationStreamId, UserId};
use systemprompt_models::issues::{IssueReport, IssueStatus};
use systemprompt_models::notifications::{
    Attention, NotificationEvent, NotificationKind, SubjectRef,
};

use super::error::{IssuesError, IssuesResult};
pub use super::filing::{IssueFiling, fingerprint};
use super::filing::{MAX_NOTE_CHARS, bounded, validated};
use super::repository::{Filed, IssueListQuery, IssueReportRepository, NewIssue};
use crate::config::IssuesConfig;

pub const ISSUE_SUBJECT: &str = "issue_report";
pub const ISSUES_STREAM: &str = "issues";


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FiledIssue {
    pub inserted: bool,
    pub occurrences: i32,
}

#[derive(Clone)]
pub struct IssueService {
    repo: IssueReportRepository,
    notifications: DynNotificationSink,
    config: IssuesConfig,
}

impl std::fmt::Debug for IssueService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IssueService")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl IssueService {
    #[must_use]
    pub const fn new(
        repo: IssueReportRepository,
        notifications: DynNotificationSink,
        config: IssuesConfig,
    ) -> Self {
        Self {
            repo,
            notifications,
            config,
        }
    }

    #[must_use]
    pub const fn config(&self) -> &IssuesConfig {
        &self.config
    }

    #[must_use]
    pub fn is_triager(&self, roles: &[String]) -> bool {
        self.config.is_triager(roles)
    }

    const fn enabled(&self) -> IssuesResult<()> {
        if self.config.is_enabled() {
            Ok(())
        } else {
            Err(IssuesError::Disabled)
        }
    }

    pub async fn file(&self, filing: IssueFiling) -> IssuesResult<(IssueReportId, FiledIssue)> {
        self.enabled()?;
        let issue = validated(filing)?;
        let limit = self.config.max_reports_per_hour;
        let mut tx = self.repo.write_pool().begin().await?;
        let (id, filed) = IssueReportRepository::file(&mut tx, &issue).await?;
        if filed.inserted {
            if IssueReportRepository::filed_last_hour(&mut tx, &issue.reporter).await?
                > i64::from(limit)
            {
                tx.rollback().await?;
                return Err(IssuesError::RateLimited { limit });
            }
            self.announce_filed(&mut tx, &id, &issue).await?;
        }
        tx.commit().await?;
        let Filed {
            inserted,
            occurrences,
        } = filed;
        Ok((
            id,
            FiledIssue {
                inserted,
                occurrences,
            },
        ))
    }

    async fn announce_filed(
        &self,
        conn: &mut sqlx::PgConnection,
        id: &IssueReportId,
        issue: &NewIssue,
    ) -> IssuesResult<()> {
        if !self.notifications.is_available() {
            tracing::debug!(issue_id = %id, "Notifications unavailable; issue report not announced");
            return Ok(());
        }
        // JSON: the notification event's payload is its free-form contract field.
        let payload = serde_json::json!({
            "kind": issue.kind,
            "severity": issue.severity,
            "title": issue.title,
            "via_agent": issue.via_agent,
        });
        let mut event = NotificationEvent::new(
            NotificationKind::new(NotificationKind::ISSUE_REPORTED)?,
            issue.reporter.clone(),
            Utc::now(),
        )
        .with_stream(NotificationStreamId::new(ISSUES_STREAM))
        .with_subject(SubjectRef::new(ISSUE_SUBJECT, id.as_str()))
        .with_excerpt(issue.title.clone())
        .with_payload(payload);
        if let Some(agent) = &issue.via_agent {
            event = event.with_via_agent(agent.clone());
        }
        self.notifications.emit(conn, &event).await?;
        Ok(())
    }

    pub async fn attach(
        &self,
        id: &IssueReportId,
        upload: &DocumentUploadId,
        reporter: &UserId,
    ) -> IssuesResult<()> {
        self.enabled()?;
        let limit = self.config.max_attachments;
        let mut tx = self.repo.write_pool().begin().await?;
        let Some((active, held)) =
            IssueReportRepository::lock_active(&mut tx, id, reporter).await?
        else {
            return Err(IssuesError::NotFound(id.clone()));
        };
        if !active {
            return Err(IssuesError::NotActive(id.clone()));
        }
        if held >= i64::from(limit) {
            return Err(IssuesError::AttachmentsFull {
                report: id.clone(),
                limit,
            });
        }
        let attached =
            IssueReportRepository::attach_completed_upload(&mut tx, id, upload, reporter).await?;
        tx.commit().await?;
        if attached
            || self
                .repo
                .attachments(id)
                .await?
                .iter()
                .any(|a| &a.upload_id == upload)
        {
            Ok(())
        } else {
            Err(IssuesError::UploadNotReady(upload.clone()))
        }
    }

    pub async fn find_visible(
        &self,
        id: &IssueReportId,
        reader: &UserId,
        roles: &[String],
    ) -> IssuesResult<IssueReport> {
        self.enabled()?;
        match self.repo.find(id).await? {
            Some(report) if &report.reporter_user_id == reader || self.is_triager(roles) => {
                Ok(report)
            },
            _ => Err(IssuesError::NotFound(id.clone())),
        }
    }

    pub async fn find(&self, id: &IssueReportId) -> IssuesResult<Option<IssueReport>> {
        Ok(self.repo.find(id).await?)
    }

    pub async fn list(&self, query: &IssueListQuery) -> IssuesResult<Vec<IssueReport>> {
        self.enabled()?;
        Ok(self.repo.list(query).await?)
    }

    pub async fn set_status(
        &self,
        id: &IssueReportId,
        status: IssueStatus,
        note: Option<String>,
        by: &UserId,
    ) -> IssuesResult<IssueStatus> {
        self.enabled()?;
        if let Some(note) = &note {
            bounded("note", note, MAX_NOTE_CHARS)?;
        }
        let mut tx = self.repo.write_pool().begin().await?;
        let change =
            IssueReportRepository::set_status(&mut tx, id, status, note.as_deref(), by).await?;
        let Some(change) = change else {
            return Err(IssuesError::NotFound(id.clone()));
        };
        if change.from != status && &change.reporter != by && self.notifications.is_available() {
            // JSON: the notification event's payload is its free-form contract field.
            let payload = serde_json::json!({
                "from": change.from,
                "to": status,
                "title": change.title,
                "note": note,
            });
            let event = NotificationEvent::new(
                NotificationKind::new(NotificationKind::ISSUE_STATUS_CHANGED)?,
                by.clone(),
                Utc::now(),
            )
            .with_subject(SubjectRef::new(ISSUE_SUBJECT, id.as_str()))
            .with_direct_recipient(change.reporter.clone(), Some(Attention::Agent))
            .with_excerpt(change.title.clone())
            .with_payload(payload);
            self.notifications.emit(&mut tx, &event).await?;
        }
        tx.commit().await?;
        Ok(change.from)
    }

    pub async fn purge_closed(&self, limit: i64) -> IssuesResult<u64> {
        Ok(self
            .repo
            .purge_closed(self.config.retention_days, limit)
            .await?)
    }
}
