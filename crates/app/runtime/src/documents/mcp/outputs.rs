//! Tool outputs. Every document location is a `document://` URI so an
//! agent can quote it back precisely.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::Serialize;
use systemprompt_identifiers::{
    ContentHash, DocumentFolderId, DocumentId, DocumentRevisionId, UserId,
};
use systemprompt_mcp::McpOutputSchema;
use systemprompt_models::documents::{AccessLevel, ActorKind, DocumentFormat, DocumentUri};

macro_rules! documents_output {
    ($ty:ty, $name:literal) => {
        impl McpOutputSchema for $ty {
            fn artifact_type() -> &'static str {
                $name
            }
        }
    };
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FolderItem {
    pub folder_id: DocumentFolderId,
    pub parent_id: Option<DocumentFolderId>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DocumentItem {
    pub uri: DocumentUri,
    pub document_id: DocumentId,
    pub folder_id: Option<DocumentFolderId>,
    pub name: String,
    pub format: DocumentFormat,
    pub head_revision_id: Option<DocumentRevisionId>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ListOutput {
    pub folders: Vec<FolderItem>,
    pub items: Vec<DocumentItem>,
    pub next_cursor: Option<String>,
}
documents_output!(ListOutput, "documents_list");

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SearchHit {
    pub uri: DocumentUri,
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
documents_output!(SearchOutput, "documents_search");

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CreateOutput {
    pub uri: DocumentUri,
    pub document_id: DocumentId,
    pub revision_id: DocumentRevisionId,
    pub format: DocumentFormat,
    pub preview_url: String,
}
documents_output!(CreateOutput, "documents_create");

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReadOutput {
    pub uri: DocumentUri,
    pub revision_id: DocumentRevisionId,
    pub content_sha256: ContentHash,
    pub format: DocumentFormat,
    pub lines: Vec<String>,
    pub truncated: bool,
}
documents_output!(ReadOutput, "documents_read");

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct EditOutput {
    pub uri: DocumentUri,
    pub revision_id: DocumentRevisionId,
    pub parent_revision_id: Option<DocumentRevisionId>,
    pub applied: usize,
    pub merged: bool,
    pub changed_nodes: Vec<DocumentUri>,
    pub reread_hint: Option<String>,
}
documents_output!(EditOutput, "documents_edit");

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RevisionItem {
    pub uri: DocumentUri,
    pub revision_id: DocumentRevisionId,
    pub parent_revision_id: Option<DocumentRevisionId>,
    pub author_user_id: UserId,
    pub actor_kind: ActorKind,
    pub message: Option<String>,
    pub content_sha256: ContentHash,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct HistoryOutput {
    pub revisions: Vec<RevisionItem>,
}
documents_output!(HistoryOutput, "documents_history");

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ShareOutput {
    pub grant_id: String,
    pub subject_kind: String,
    pub subject_value: String,
    pub level: AccessLevel,
    pub valid_until: Option<DateTime<Utc>>,
}
documents_output!(ShareOutput, "documents_share");

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DownloadOutput {
    pub uri: DocumentUri,
    pub revision_id: DocumentRevisionId,
    pub url: String,
    pub expires_at: DateTime<Utc>,
}
documents_output!(DownloadOutput, "documents_download_url");

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PreviewOutput {
    pub uri: DocumentUri,
    pub revision_id: DocumentRevisionId,
    pub html: String,
    pub ui_resource: String,
}
documents_output!(PreviewOutput, "documents_render_preview");
