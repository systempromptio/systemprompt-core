//! The JSON bodies and query parameters of the issue routes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ContentHash, DocumentUploadId, IssueReportId, UserId};
use systemprompt_models::api::ApiError;
use systemprompt_models::issues::{IssueKind, IssueReport, IssueSeverity, IssueStatus};

use crate::error::ApiHttpError;

const DEFAULT_LIMIT: u32 = 50;

const MAX_LIMIT: u32 = 100;

#[derive(Debug, Deserialize)]
pub(super) struct FileBody {
    pub(super) kind: IssueKind,
    pub(super) severity: IssueSeverity,
    pub(super) title: String,
    #[serde(default)]
    pub(super) body: Option<String>,
    #[serde(default)]
    pub(super) fingerprint: Option<String>,
    #[serde(default)]
    // JSON: the client block is the reporter's free-form environment description.
    pub(super) client: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub(super) struct FiledJson {
    pub(super) id: IssueReportId,
    pub(super) folded: bool,
    pub(super) occurrences: i32,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct ListParams {
    #[serde(default)]
    pub(super) status: Option<IssueStatus>,
    #[serde(default)]
    pub(super) reporter: Option<String>,
    #[serde(default)]
    pub(super) limit: Option<u32>,
    #[serde(default)]
    pub(super) before_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub(super) before_id: Option<String>,
}

impl ListParams {
    pub(super) fn limit(&self) -> u32 {
        self.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
    }

    #[expect(
        clippy::result_large_err,
        reason = "ApiHttpError is the handler error type; boxing it here would propagate to every \
                  caller for negligible gain"
    )]
    pub(super) fn before(&self) -> Result<Option<(DateTime<Utc>, IssueReportId)>, ApiHttpError> {
        match (self.before_at, self.before_id.as_deref()) {
            (None, None) => Ok(None),
            (Some(at), Some(id)) => Ok(Some((at, IssueReportId::try_new(id)?))),
            _ => Err(ApiError::bad_request("before_at and before_id travel together").into()),
        }
    }

    #[expect(
        clippy::result_large_err,
        reason = "ApiHttpError is the handler error type; boxing it here would propagate to every \
                  caller for negligible gain"
    )]
    pub(super) fn reporter(&self) -> Result<Option<UserId>, ApiHttpError> {
        Ok(self.reporter.as_deref().map(UserId::try_new).transpose()?)
    }
}

#[derive(Debug, Serialize)]
pub(super) struct Cursor {
    before_at: DateTime<Utc>,
    before_id: IssueReportId,
}

#[derive(Debug, Serialize)]
pub(super) struct IssuePage {
    issues: Vec<IssueReport>,
    next: Option<Cursor>,
}

impl IssuePage {
    pub(super) fn new(issues: Vec<IssueReport>, limit: u32) -> Self {
        let full = usize::try_from(limit).is_ok_and(|limit| issues.len() >= limit);
        let next = issues.last().filter(|_| full).map(|last| Cursor {
            before_at: last.last_seen_at,
            before_id: last.id.clone(),
        });
        Self { issues, next }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct StatusBody {
    pub(super) status: IssueStatus,
    #[serde(default)]
    pub(super) note: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct StatusJson {
    pub(super) from: IssueStatus,
    pub(super) to: IssueStatus,
}

#[derive(Debug, Deserialize)]
pub(super) struct AttachmentBody {
    pub(super) file_name: String,
    pub(super) mime: String,
    pub(super) size: u64,
    pub(super) sha256: ContentHash,
}

#[derive(Debug, Serialize)]
pub(super) struct AttachmentUploadJson {
    pub(super) upload_id: DocumentUploadId,
    pub(super) part_size: u64,
    pub(super) part_count: u32,
    pub(super) parts_url: String,
    pub(super) status_url: String,
    pub(super) complete_url: String,
    pub(super) upload_token: String,
    pub(super) token_header: &'static str,
    pub(super) expires_at: DateTime<Utc>,
}
