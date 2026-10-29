//! Tool outputs, terse by default: each carries the ids an agent acts on
//! next and nothing it already holds. `verbose: true` on a read, edit,
//! history or list adds the `document://` URIs and content hashes. A write
//! acknowledges what it wrote as read lines, so the next edit anchors on
//! the ack without a re-read. Outputs reach the wire once, as structured
//! content. A history item names its author as `author`, its actor kind as
//! `actor` and its time as `at`, to the second.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::Serialize;
use systemprompt_identifiers::{
    ContentHash, ContextId, DocumentFolderId, DocumentId, DocumentProposalId, DocumentRevisionId,
    SessionId, TaskId, TraceId, UserId,
};
use systemprompt_mcp::McpOutputSchema;

use crate::documents::views::{EditOutput, GrantItem, ReadOutput};
use systemprompt_models::documents::{
    AccessLevel, ActorKind, DocumentFormat, DocumentUri, ProposalStatus,
};

macro_rules! documents_output_impl {
    ($ty:ty, $name:literal) => {
        impl McpOutputSchema for $ty {
            fn artifact_type() -> &'static str {
                $name
            }

            fn terse_wire(&self) -> bool {
                true
            }
        }
    };
}

pub(super) use documents_output_impl as documents_output;

fn seconds<S: serde::Serializer>(at: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FolderItem {
    pub folder_id: DocumentFolderId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<DocumentFolderId>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DocumentItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<DocumentUri>,
    pub document_id: DocumentId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<DocumentFolderId>,
    pub name: String,
    pub format: DocumentFormat,
    pub head_revision_id: Option<DocumentRevisionId>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub my_access: Option<AccessLevel>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ListOutput {
    pub folders: Vec<FolderItem>,
    pub items: Vec<DocumentItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}
documents_output_impl!(ListOutput, "documents_list");

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SearchHit {
    pub document_id: DocumentId,
    pub revision_id: DocumentRevisionId,
    pub name: String,
    pub format: DocumentFormat,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SearchOutput {
    pub hits: Vec<SearchHit>,
}
documents_output_impl!(SearchOutput, "documents_search");

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CreateOutput {
    pub document_id: DocumentId,
    pub revision_id: DocumentRevisionId,
    pub format: DocumentFormat,
    pub name: String,
    pub written: Vec<String>,
}
documents_output_impl!(CreateOutput, "documents_create");
documents_output_impl!(ReadOutput, "documents_read");
documents_output_impl!(EditOutput, "documents_edit");

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RevisionItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<DocumentUri>,
    pub revision_id: DocumentRevisionId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_revision_id: Option<DocumentRevisionId>,
    #[serde(rename = "author")]
    pub author_user_id: UserId,
    #[serde(rename = "actor")]
    pub actor_kind: ActorKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_sha256: Option<ContentHash>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attribution: Option<RevisionAttributionItem>,
    #[serde(rename = "at", serialize_with = "seconds")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RevisionAttributionItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<SessionId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_id: Option<ContextId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<TraceId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_behalf_of: Option<UserId>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct HistoryOutput {
    pub revisions: Vec<RevisionItem>,
}
documents_output_impl!(HistoryOutput, "documents_history");

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ShareOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grant: Option<GrantItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transferred_to: Option<UserId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revoked: Option<u64>,
}
documents_output_impl!(ShareOutput, "documents_share");

/// `access` is the level held on the asked document, absent for none or
/// when no document was asked.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PersonItem {
    pub user_id: UserId,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access: Option<AccessLevel>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PeopleOutput {
    pub people: Vec<PersonItem>,
}
documents_output_impl!(PeopleOutput, "documents_people");

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ExportOutput {
    pub revision_id: DocumentRevisionId,
    pub format: DocumentFormat,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui_resource: Option<String>,
}
documents_output_impl!(ExportOutput, "documents_export");

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct GuideOutput {
    pub topics: Vec<String>,
    #[schemars(
        description = "The guide text itself. A host that shows the model only the structured content (Claude Code does) would otherwise see the topic names and nothing else."
    )]
    pub guide: String,
}
documents_output_impl!(GuideOutput, "documents_guide");

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ProposalItem {
    pub proposal_id: DocumentProposalId,
    pub status: ProposalStatus,
    pub proposer: UserId,
    pub actor_kind: ActorKind,
    pub base_revision_id: DocumentRevisionId,
    pub proposed_revision_id: DocumentRevisionId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ProposalsOutput {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub proposals: Vec<ProposalItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewed: Option<ReviewOutput>,
}
documents_output_impl!(ProposalsOutput, "documents_proposals");

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReviewOutput {
    pub proposal_id: DocumentProposalId,
    pub status: ProposalStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision_id: Option<DocumentRevisionId>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub rebased: bool,
}
