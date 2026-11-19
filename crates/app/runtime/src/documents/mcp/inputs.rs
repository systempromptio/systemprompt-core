//! Tool inputs: one struct per tool, each rendered as an object
//! `inputSchema`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::Deserialize;
use systemprompt_files::documents::service::MentionOption;
use systemprompt_identifiers::{
    ContentHash, ContextId, DocumentCommentId, DocumentFolderId, DocumentId, DocumentNodeId,
    DocumentOpId, DocumentRevisionId, DocumentTemplateId, UserId,
};
use systemprompt_models::documents::{AccessLevel, DocumentFormat, EditMode, EditOp};
use systemprompt_models::notifications::Attention;

pub use super::lifecycle_inputs::{
    CopyInput, FolderAction, FolderInput, ImportInput, ProposalAction, ProposalsInput, UpdateInput,
};

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ListInput {
    #[serde(default)]
    pub scope: ListScopeInput,
    #[serde(default)]
    pub folder_id: Option<DocumentFolderId>,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub sort: ListSortInput,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub verbose: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ListScopeInput {
    #[default]
    Folder,
    Shared,
    Mine,
    Recent,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ListSortInput {
    Name,
    #[default]
    Updated,
    Recent,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct SearchInput {
    pub query: String,
    #[serde(default)]
    pub format: Option<DocumentFormat>,
    #[serde(default)]
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CreateInput {
    pub template_id: DocumentTemplateId,
    #[serde(default)]
    pub folder_id: Option<DocumentFolderId>,
    pub name: String,
    #[serde(default)]
    pub params: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    // JSON: `{model}` or `{markdown}`, untyped for size; create_content checks it.
    pub content: Option<serde_json::Value>,
    #[serde(default, deserialize_with = "super::op_inputs::ops")]
    pub ops: Vec<EditOp>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub context_id: Option<ContextId>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct EditInput {
    pub document_id: DocumentId,
    #[serde(default)]
    pub base_revision: Option<DocumentRevisionId>,
    #[serde(default, deserialize_with = "super::op_inputs::ops")]
    pub ops: Vec<EditOp>,
    #[serde(default)]
    pub content_blob_ref: Option<ContentHash>,
    #[serde(default)]
    pub mode: Option<EditMode>,
    #[serde(default)]
    pub op_id: Option<DocumentOpId>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub answering: Option<DocumentCommentId>,
    #[serde(default)]
    pub allow_detach: bool,
    #[serde(default)]
    pub verbose: bool,
    #[serde(default)]
    pub context_id: Option<ContextId>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct HistoryInput {
    pub document_id: DocumentId,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub verbose: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ShareInput {
    #[serde(default)]
    pub document_id: Option<DocumentId>,
    #[serde(default)]
    pub folder_id: Option<DocumentFolderId>,
    #[serde(default)]
    pub subject_kind: Option<String>,
    #[serde(default)]
    pub subject_value: Option<String>,
    #[serde(default)]
    pub level: Option<AccessLevel>,
    #[serde(default)]
    pub valid_until: Option<DateTime<Utc>>,
    #[serde(default)]
    pub transfer_to: Option<UserId>,
    #[serde(default)]
    pub revoke_all: bool,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    #[default]
    Native,
    Pdf,
    Md,
    Html,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExportAs {
    #[default]
    Url,
    Preview,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ExportInput {
    pub document_id: DocumentId,
    #[serde(default)]
    pub revision_id: Option<DocumentRevisionId>,
    #[serde(default)]
    pub format: ExportFormat,
    #[serde(default, rename = "as")]
    pub export_as: ExportAs,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct GuideInput {
    pub topics: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CommentAction {
    Create,
    List,
    Resolve,
    Reopen,
    Edit,
    Delete,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct QuoteAnchor {
    pub find: String,
    #[serde(default)]
    pub nth: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct MentionOptionInput {
    pub user_id: UserId,
    pub attention: Attention,
}

impl From<MentionOptionInput> for MentionOption {
    fn from(input: MentionOptionInput) -> Self {
        Self {
            user_id: input.user_id,
            attention: input.attention,
        }
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CommentInput {
    pub action: CommentAction,
    pub document_id: DocumentId,
    #[serde(default)]
    pub node: Option<DocumentNodeId>,
    #[serde(default)]
    pub anchor: Option<QuoteAnchor>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub parent_id: Option<DocumentCommentId>,
    #[serde(default)]
    pub comment_id: Option<DocumentCommentId>,
    #[serde(default)]
    pub answering: Option<DocumentRevisionId>,
    #[serde(default)]
    pub mentions: Vec<UserId>,
    #[serde(default)]
    pub mention_options: Vec<MentionOptionInput>,
    #[serde(default)]
    pub context_id: Option<ContextId>,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct PeopleInput {
    pub query: String,
    #[serde(default)]
    pub document_id: Option<DocumentId>,
    #[serde(default)]
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct DocumentInput {
    pub document_id: DocumentId,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct AccessInput {
    pub document_id: DocumentId,
    #[serde(default)]
    pub history: bool,
}
