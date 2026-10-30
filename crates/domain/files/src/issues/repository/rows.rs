//! The issue report row types: what filing takes and returns, the listing
//! query, a triage transition, and the decoding of a stored row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::str::FromStr;

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{
    AgentName, ContextId, IssueReportId, McpExecutionId, SessionId, TaskId, TraceId, UserId,
};
use systemprompt_models::issues::{
    IssueContext, IssueKind, IssueReport, IssueSeverity, IssueStatus,
};
use systemprompt_traits::RepositoryError;

pub(super) type RepoResult<T> = Result<T, RepositoryError>;

pub(super) fn parse<T>(field: &'static str, value: &str) -> RepoResult<T>
where
    T: FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    T::from_str(value).map_err(|source| RepositoryError::decode(field, source))
}

/// A report to file, with the fingerprint that folds its repeats.
#[derive(Debug, Clone)]
pub struct NewIssue {
    pub reporter: UserId,
    pub via_agent: Option<AgentName>,
    pub kind: IssueKind,
    pub severity: IssueSeverity,
    pub title: String,
    pub body: String,
    pub fingerprint: String,
    pub context: IssueContext,
    // JSON: the reporter's client facts are a free-form, size-bounded object.
    pub client: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Filed {
    pub inserted: bool,
    pub occurrences: i32,
}

/// A keyset listing request, most recently seen first.
#[derive(Debug, Clone, Default)]
pub struct IssueListQuery {
    pub reporter: Option<UserId>,
    pub status: Option<IssueStatus>,
    pub before: Option<(DateTime<Utc>, IssueReportId)>,
    pub limit: u32,
}

/// What a triage transition changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusChange {
    pub reporter: UserId,
    pub from: IssueStatus,
    pub title: String,
}

pub(super) struct ReportRow {
    pub(super) id: IssueReportId,
    pub(super) reporter_user_id: UserId,
    pub(super) via_agent: Option<String>,
    pub(super) kind: String,
    pub(super) severity: String,
    pub(super) status: String,
    pub(super) title: String,
    pub(super) body: String,
    pub(super) occurrences: i32,
    pub(super) trace_id: Option<TraceId>,
    pub(super) context_id: Option<ContextId>,
    pub(super) session_id: Option<SessionId>,
    pub(super) task_id: Option<TaskId>,
    pub(super) mcp_execution_id: Option<McpExecutionId>,
    // JSON: the stored client facts column is free-form JSONB.
    pub(super) client: serde_json::Value,
    pub(super) triage_note: Option<String>,
    pub(super) triaged_by: Option<UserId>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) last_seen_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
    pub(super) closed_at: Option<DateTime<Utc>>,
}

impl ReportRow {
    pub(super) fn into_report(self) -> RepoResult<IssueReport> {
        Ok(IssueReport {
            id: self.id,
            reporter_user_id: self.reporter_user_id,
            via_agent: self.via_agent.map(AgentName::new),
            kind: parse("issue_reports.kind", &self.kind)?,
            severity: parse("issue_reports.severity", &self.severity)?,
            status: parse("issue_reports.status", &self.status)?,
            title: self.title,
            body: self.body,
            occurrences: self.occurrences,
            context: IssueContext {
                trace: self.trace_id,
                context: self.context_id,
                session: self.session_id,
                task: self.task_id,
                mcp_execution: self.mcp_execution_id,
            },
            client: self.client,
            triage_note: self.triage_note,
            triaged_by: self.triaged_by,
            created_at: self.created_at,
            last_seen_at: self.last_seen_at,
            updated_at: self.updated_at,
            closed_at: self.closed_at,
            attachments: Vec::new(),
        })
    }
}
