//! Issue report contracts: the bug and feedback reports a user or an agent
//! files, the context that ties a report to the platform's own traces, and
//! the triage vocabulary.
//!
//! A report carries identifiers, never copied logs: `trace_id`,
//! `context_id`, `session_id`, `task_id` and `mcp_execution_id` name the
//! records the platform already keeps, so triage follows them to the trace
//! and the report holds no secret a log line might.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    ContentHash, ContextId, DocumentUploadId, IssueReportId, McpExecutionId, SessionId, TaskId,
    TraceId, UserId,
};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown {kind} `{value}`")]
pub struct UnknownIssueValue {
    pub kind: &'static str,
    pub value: String,
}

macro_rules! issue_enum {
    ($name:ident, $kind:literal, { $($variant:ident => $wire:literal),+ $(,)? }) => {
        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire),+
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = UnknownIssueValue;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($wire => Ok(Self::$variant),)+
                    other => Err(UnknownIssueValue {
                        kind: $kind,
                        value: other.to_owned(),
                    }),
                }
            }
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IssueKind {
    Bug,
    Feedback,
}

issue_enum!(IssueKind, "issue kind", {
    Bug => "bug",
    Feedback => "feedback",
});

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum IssueSeverity {
    Low,
    Medium,
    High,
    Critical,
}

issue_enum!(IssueSeverity, "issue severity", {
    Low => "low",
    Medium => "medium",
    High => "high",
    Critical => "critical",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IssueStatus {
    Open,
    Triaged,
    NeedsInfo,
    Resolved,
    Closed,
}

issue_enum!(IssueStatus, "issue status", {
    Open => "open",
    Triaged => "triaged",
    NeedsInfo => "needs_info",
    Resolved => "resolved",
    Closed => "closed",
});

impl IssueStatus {
    #[must_use]
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Open | Self::Triaged | Self::NeedsInfo)
    }
}

/// The platform records a report points at, captured from the filing
/// request rather than supplied by the reporter.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IssueContext {
    pub trace: Option<TraceId>,
    pub context: Option<ContextId>,
    pub session: Option<SessionId>,
    pub task: Option<TaskId>,
    pub mcp_execution: Option<McpExecutionId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IssueAttachment {
    pub upload_id: DocumentUploadId,
    pub content_sha256: ContentHash,
    pub file_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub created_at: DateTime<Utc>,
}

/// One report as its reporter and triage see it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct IssueReport {
    pub id: IssueReportId,
    pub reporter_user_id: UserId,
    pub via_agent: Option<String>,
    pub kind: IssueKind,
    pub severity: IssueSeverity,
    pub status: IssueStatus,
    pub title: String,
    pub body: String,
    pub occurrences: i32,
    pub context: IssueContext,
    pub client: serde_json::Value,
    pub triage_note: Option<String>,
    pub triaged_by: Option<UserId>,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub attachments: Vec<IssueAttachment>,
}
